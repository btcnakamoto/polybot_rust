use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::Whale;

/// Insert a new whale or return existing one by address.
pub async fn upsert_whale(pool: &PgPool, address: &str) -> anyhow::Result<Whale> {
    let whale = sqlx::query_as::<_, Whale>(
        r#"
        INSERT INTO whales (address)
        VALUES ($1)
        ON CONFLICT (address) DO UPDATE SET updated_at = NOW()
        RETURNING *
        "#,
    )
    .bind(address)
    .fetch_one(pool)
    .await?;

    Ok(whale)
}

/// Fetch a whale by its wallet address.
pub async fn get_whale_by_address(pool: &PgPool, address: &str) -> anyhow::Result<Option<Whale>> {
    let whale = sqlx::query_as::<_, Whale>(
        "SELECT * FROM whales WHERE address = $1",
    )
    .bind(address)
    .fetch_optional(pool)
    .await?;

    Ok(whale)
}

/// Fetch all active whales.
pub async fn get_active_whales(pool: &PgPool) -> anyhow::Result<Vec<Whale>> {
    let whales = sqlx::query_as::<_, Whale>(
        "SELECT * FROM whales WHERE is_active = true ORDER BY updated_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(whales)
}

/// Update scoring metrics for a whale.
#[allow(clippy::too_many_arguments)]
pub async fn update_whale_scores(
    pool: &PgPool,
    whale_id: Uuid,
    sharpe_ratio: Decimal,
    win_rate: Decimal,
    kelly_fraction: Decimal,
    expected_value: Decimal,
    total_trades: i32,
    total_pnl: Decimal,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE whales
        SET sharpe_ratio = $2,
            win_rate = $3,
            kelly_fraction = $4,
            expected_value = $5,
            total_trades = $6,
            total_pnl = $7,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(whale_id)
    .bind(sharpe_ratio)
    .bind(win_rate)
    .bind(kelly_fraction)
    .bind(expected_value)
    .bind(total_trades)
    .bind(total_pnl)
    .execute(pool)
    .await?;

    Ok(())
}

/// Update classification for a whale.
pub async fn update_whale_classification(
    pool: &PgPool,
    whale_id: Uuid,
    classification: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE whales SET classification = $2, updated_at = NOW() WHERE id = $1",
    )
    .bind(whale_id)
    .bind(classification)
    .execute(pool)
    .await?;

    Ok(())
}

/// Deactivate a whale (stop copying).
pub async fn deactivate_whale(pool: &PgPool, whale_id: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE whales SET is_active = false, updated_at = NOW() WHERE id = $1",
    )
    .bind(whale_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Deactivate whales that haven't traded in `max_inactive_days` days.
/// Returns the number of whales deactivated.
pub async fn deactivate_stale_whales(pool: &PgPool, max_inactive_days: i64) -> anyhow::Result<u64> {
    let result = sqlx::query(
        r#"
        UPDATE whales SET is_active = false, updated_at = NOW()
        WHERE is_active = true
          AND (
            (last_trade_at IS NOT NULL AND last_trade_at < NOW() - make_interval(days => $1))
            OR
            (last_trade_at IS NULL AND created_at < NOW() - make_interval(days => $1))
          )
        "#,
    )
    .bind(max_inactive_days as i32)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// Get all whale addresses (active and inactive).
pub async fn get_all_whale_addresses(pool: &PgPool) -> anyhow::Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT address FROM whales")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// Update the last_trade_at timestamp.
pub async fn touch_whale_last_trade(
    pool: &PgPool,
    whale_id: Uuid,
    traded_at: DateTime<Utc>,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE whales SET last_trade_at = $2, updated_at = NOW() WHERE id = $1",
    )
    .bind(whale_id)
    .bind(traded_at)
    .execute(pool)
    .await?;

    Ok(())
}
