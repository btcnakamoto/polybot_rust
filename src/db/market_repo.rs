use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::MarketOutcome;

/// Insert a market_outcome record if it doesn't exist.
pub async fn upsert_market_outcome(
    pool: &PgPool,
    market_id: &str,
    token_id: Option<&str>,
) -> anyhow::Result<MarketOutcome> {
    let row = sqlx::query_as::<_, MarketOutcome>(
        r#"
        INSERT INTO market_outcomes (market_id, token_id)
        VALUES ($1, $2)
        ON CONFLICT (market_id) DO UPDATE SET updated_at = NOW()
        RETURNING *
        "#,
    )
    .bind(market_id)
    .bind(token_id)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Mark a market as resolved with the given outcome string.
pub async fn resolve_market(
    pool: &PgPool,
    market_id: &str,
    outcome_str: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE market_outcomes
        SET outcome = $2, resolved_at = $3, updated_at = NOW()
        WHERE market_id = $1
        "#,
    )
    .bind(market_id)
    .bind(outcome_str)
    .bind(Utc::now())
    .execute(pool)
    .await?;

    Ok(())
}

/// Get all markets that have not yet resolved.
pub async fn get_unresolved_markets(pool: &PgPool) -> anyhow::Result<Vec<MarketOutcome>> {
    let rows = sqlx::query_as::<_, MarketOutcome>(
        "SELECT * FROM market_outcomes WHERE outcome = 'unresolved'",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Get liquidity for a market by condition_id from active_markets table.
pub async fn get_market_liquidity(pool: &PgPool, condition_id: &str) -> anyhow::Result<Option<Decimal>> {
    let row: Option<(Decimal,)> = sqlx::query_as(
        "SELECT liquidity FROM active_markets WHERE condition_id = $1",
    )
    .bind(condition_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.0))
}

/// Get a single market outcome by market_id.
pub async fn get_market_outcome(
    pool: &PgPool,
    market_id: &str,
) -> anyhow::Result<Option<MarketOutcome>> {
    let row = sqlx::query_as::<_, MarketOutcome>(
        "SELECT * FROM market_outcomes WHERE market_id = $1",
    )
    .bind(market_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}
