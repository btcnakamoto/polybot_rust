use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{ConsensusSignal, Whale, WhaleBasket};

/// Vote cast by a whale in the consensus window.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BasketTradeVote {
    pub whale_id: Uuid,
    pub side: String,
    pub traded_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Basket CRUD
// ---------------------------------------------------------------------------

pub async fn create_basket(
    pool: &PgPool,
    name: &str,
    category: &str,
    consensus_threshold: Decimal,
    time_window_hours: i32,
    min_wallets: i32,
    max_wallets: i32,
) -> anyhow::Result<WhaleBasket> {
    let basket = sqlx::query_as::<_, WhaleBasket>(
        r#"
        INSERT INTO whale_baskets (name, category, consensus_threshold, time_window_hours, min_wallets, max_wallets)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING *
        "#,
    )
    .bind(name)
    .bind(category)
    .bind(consensus_threshold)
    .bind(time_window_hours)
    .bind(min_wallets)
    .bind(max_wallets)
    .fetch_one(pool)
    .await?;

    Ok(basket)
}

pub async fn get_active_baskets(pool: &PgPool) -> anyhow::Result<Vec<WhaleBasket>> {
    let baskets = sqlx::query_as::<_, WhaleBasket>(
        "SELECT * FROM whale_baskets WHERE is_active = true ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(baskets)
}

pub async fn get_basket_by_id(pool: &PgPool, id: Uuid) -> anyhow::Result<Option<WhaleBasket>> {
    let basket = sqlx::query_as::<_, WhaleBasket>(
        "SELECT * FROM whale_baskets WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(basket)
}

pub async fn deactivate_basket(pool: &PgPool, id: Uuid) -> anyhow::Result<()> {
    sqlx::query("UPDATE whale_baskets SET is_active = false, updated_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn count_active_baskets(pool: &PgPool) -> anyhow::Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM whale_baskets WHERE is_active = true",
    )
    .fetch_one(pool)
    .await?;

    Ok(row.0)
}

// ---------------------------------------------------------------------------
// Basket membership
// ---------------------------------------------------------------------------

pub async fn add_whale_to_basket(
    pool: &PgPool,
    basket_id: Uuid,
    whale_id: Uuid,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO basket_wallets (basket_id, whale_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(basket_id)
    .bind(whale_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn remove_whale_from_basket(
    pool: &PgPool,
    basket_id: Uuid,
    whale_id: Uuid,
) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM basket_wallets WHERE basket_id = $1 AND whale_id = $2")
        .bind(basket_id)
        .bind(whale_id)
        .execute(pool)
        .await?;

    Ok(())
}

/// Get all whales in a basket (JOIN with whales table).
pub async fn get_basket_whales(pool: &PgPool, basket_id: Uuid) -> anyhow::Result<Vec<Whale>> {
    let whales = sqlx::query_as::<_, Whale>(
        r#"
        SELECT w.* FROM whales w
        INNER JOIN basket_wallets bw ON bw.whale_id = w.id
        WHERE bw.basket_id = $1
        ORDER BY w.win_rate DESC NULLS LAST
        "#,
    )
    .bind(basket_id)
    .fetch_all(pool)
    .await?;

    Ok(whales)
}

pub async fn count_basket_whales(pool: &PgPool, basket_id: Uuid) -> anyhow::Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM basket_wallets WHERE basket_id = $1",
    )
    .bind(basket_id)
    .fetch_one(pool)
    .await?;

    Ok(row.0)
}

/// Get all baskets that a whale belongs to (active only).
pub async fn get_baskets_for_whale(
    pool: &PgPool,
    whale_id: Uuid,
) -> anyhow::Result<Vec<WhaleBasket>> {
    let baskets = sqlx::query_as::<_, WhaleBasket>(
        r#"
        SELECT wb.* FROM whale_baskets wb
        INNER JOIN basket_wallets bw ON bw.basket_id = wb.id
        WHERE bw.whale_id = $1 AND wb.is_active = true
        "#,
    )
    .bind(whale_id)
    .fetch_all(pool)
    .await?;

    Ok(baskets)
}

// ---------------------------------------------------------------------------
// Consensus queries
// ---------------------------------------------------------------------------

/// Core consensus query: for each whale in the basket, get their most recent
/// trade direction within the time window for a specific market.
/// Only considers whales that are is_active = true.
pub async fn get_basket_trades_in_window(
    pool: &PgPool,
    basket_id: Uuid,
    market_id: &str,
    since: DateTime<Utc>,
) -> anyhow::Result<Vec<BasketTradeVote>> {
    let votes = sqlx::query_as::<_, BasketTradeVote>(
        r#"
        SELECT DISTINCT ON (wt.whale_id) wt.whale_id, wt.side, wt.traded_at
        FROM whale_trades wt
        INNER JOIN basket_wallets bw ON bw.whale_id = wt.whale_id
        INNER JOIN whales w ON w.id = wt.whale_id
        WHERE bw.basket_id = $1
          AND wt.market_id = $2
          AND wt.traded_at >= $3
          AND w.is_active = true
        ORDER BY wt.whale_id, wt.traded_at DESC
        "#,
    )
    .bind(basket_id)
    .bind(market_id)
    .bind(since)
    .fetch_all(pool)
    .await?;

    Ok(votes)
}

// ---------------------------------------------------------------------------
// Consensus signal recording
// ---------------------------------------------------------------------------

pub async fn record_consensus_signal(
    pool: &PgPool,
    basket_id: Uuid,
    market_id: &str,
    direction: &str,
    consensus_pct: Decimal,
    participating_whales: i32,
    total_whales: i32,
) -> anyhow::Result<ConsensusSignal> {
    let signal = sqlx::query_as::<_, ConsensusSignal>(
        r#"
        INSERT INTO consensus_signals (basket_id, market_id, direction, consensus_pct, participating_whales, total_whales)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING *
        "#,
    )
    .bind(basket_id)
    .bind(market_id)
    .bind(direction)
    .bind(consensus_pct)
    .bind(participating_whales)
    .bind(total_whales)
    .fetch_one(pool)
    .await?;

    Ok(signal)
}

pub async fn get_recent_consensus_signals(
    pool: &PgPool,
    limit: i64,
) -> anyhow::Result<Vec<ConsensusSignal>> {
    let signals = sqlx::query_as::<_, ConsensusSignal>(
        "SELECT * FROM consensus_signals ORDER BY triggered_at DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(signals)
}

pub async fn get_consensus_signals_for_basket(
    pool: &PgPool,
    basket_id: Uuid,
    limit: i64,
) -> anyhow::Result<Vec<ConsensusSignal>> {
    let signals = sqlx::query_as::<_, ConsensusSignal>(
        "SELECT * FROM consensus_signals WHERE basket_id = $1 ORDER BY triggered_at DESC LIMIT $2",
    )
    .bind(basket_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(signals)
}

pub async fn count_recent_consensus_signals(
    pool: &PgPool,
    since: DateTime<Utc>,
) -> anyhow::Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM consensus_signals WHERE triggered_at >= $1",
    )
    .bind(since)
    .fetch_one(pool)
    .await?;

    Ok(row.0)
}
