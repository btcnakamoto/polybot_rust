use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::WhaleTrade;

/// Insert a new whale trade record.
pub async fn insert_trade(
    pool: &PgPool,
    whale_id: Uuid,
    market_id: &str,
    token_id: &str,
    side: &str,
    size: Decimal,
    price: Decimal,
    notional: Decimal,
    traded_at: DateTime<Utc>,
) -> anyhow::Result<WhaleTrade> {
    let trade = sqlx::query_as::<_, WhaleTrade>(
        r#"
        INSERT INTO whale_trades (whale_id, market_id, token_id, side, size, price, notional, traded_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING *
        "#,
    )
    .bind(whale_id)
    .bind(market_id)
    .bind(token_id)
    .bind(side)
    .bind(size)
    .bind(price)
    .bind(notional)
    .bind(traded_at)
    .fetch_one(pool)
    .await?;

    Ok(trade)
}

/// Get all trades for a whale, ordered by time descending.
pub async fn get_trades_by_whale(
    pool: &PgPool,
    whale_id: Uuid,
) -> anyhow::Result<Vec<WhaleTrade>> {
    let trades = sqlx::query_as::<_, WhaleTrade>(
        "SELECT * FROM whale_trades WHERE whale_id = $1 ORDER BY traded_at DESC",
    )
    .bind(whale_id)
    .fetch_all(pool)
    .await?;

    Ok(trades)
}

/// Get the N most recent trades for a whale.
pub async fn get_recent_trades(
    pool: &PgPool,
    whale_id: Uuid,
    limit: i64,
) -> anyhow::Result<Vec<WhaleTrade>> {
    let trades = sqlx::query_as::<_, WhaleTrade>(
        "SELECT * FROM whale_trades WHERE whale_id = $1 ORDER BY traded_at DESC LIMIT $2",
    )
    .bind(whale_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(trades)
}

/// Count total trades for a whale.
pub async fn count_trades(pool: &PgPool, whale_id: Uuid) -> anyhow::Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM whale_trades WHERE whale_id = $1",
    )
    .bind(whale_id)
    .fetch_one(pool)
    .await?;

    Ok(row.0)
}

/// Get trades within a time window for a whale in a specific market.
pub async fn get_trades_in_window(
    pool: &PgPool,
    whale_id: Uuid,
    market_id: &str,
    since: DateTime<Utc>,
) -> anyhow::Result<Vec<WhaleTrade>> {
    let trades = sqlx::query_as::<_, WhaleTrade>(
        r#"
        SELECT * FROM whale_trades
        WHERE whale_id = $1 AND market_id = $2 AND traded_at >= $3
        ORDER BY traded_at DESC
        "#,
    )
    .bind(whale_id)
    .bind(market_id)
    .bind(since)
    .fetch_all(pool)
    .await?;

    Ok(trades)
}
