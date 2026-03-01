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
/// Handles both `0x`-prefixed and bare hex formats.
pub async fn get_market_liquidity(pool: &PgPool, condition_id: &str) -> anyhow::Result<Option<Decimal>> {
    let row: Option<(Decimal,)> = sqlx::query_as(
        "SELECT liquidity FROM active_markets WHERE condition_id = $1",
    )
    .bind(condition_id)
    .fetch_optional(pool)
    .await?;

    if row.is_some() {
        return Ok(row.map(|r| r.0));
    }

    // Retry with 0x prefix if the original had none
    if !condition_id.starts_with("0x") {
        let prefixed = format!("0x{}", condition_id);
        let row: Option<(Decimal,)> = sqlx::query_as(
            "SELECT liquidity FROM active_markets WHERE condition_id = $1",
        )
        .bind(&prefixed)
        .fetch_optional(pool)
        .await?;
        return Ok(row.map(|r| r.0));
    }

    Ok(None)
}

/// Get the question text for a market by condition_id or token_id from active_markets.
/// Handles hex condition_ids (with/without `0x` prefix) and decimal token_ids
/// (from chain listener events).
pub async fn get_market_question(pool: &PgPool, market_id: &str) -> anyhow::Result<Option<String>> {
    // Try the ID as condition_id first
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT question FROM active_markets WHERE condition_id = $1",
    )
    .bind(market_id)
    .fetch_optional(pool)
    .await?;

    if row.is_some() {
        return Ok(row.map(|r| r.0));
    }

    // Retry with 0x prefix if the original had none
    if !market_id.starts_with("0x") {
        let prefixed = format!("0x{}", market_id);
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT question FROM active_markets WHERE condition_id = $1",
        )
        .bind(&prefixed)
        .fetch_optional(pool)
        .await?;

        if row.is_some() {
            return Ok(row.map(|r| r.0));
        }
    }

    // Fallback: search by token_id within clob_token_ids JSON array.
    // Chain listener events use decimal token_ids as market_id.
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT question FROM active_markets WHERE clob_token_ids LIKE '%' || $1 || '%'",
    )
    .bind(market_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.0))
}

/// Market info returned by get_market_info: (slug, question, clob_token_ids, outcomes).
pub type MarketInfo = (Option<String>, Option<String>, Option<String>, Option<String>);

/// Get slug, question, clob_token_ids, and outcomes for a market.
/// Handles hex condition_ids (with/without `0x` prefix) and decimal token_ids
/// (from chain listener events).
pub async fn get_market_info(pool: &PgPool, market_id: &str) -> anyhow::Result<Option<MarketInfo>> {
    let sql = "SELECT slug, question, clob_token_ids, outcomes FROM active_markets WHERE condition_id = $1";

    // Try as condition_id first
    let row: Option<MarketInfo> = sqlx::query_as(sql)
        .bind(market_id)
        .fetch_optional(pool)
        .await?;

    if row.is_some() {
        return Ok(row);
    }

    // Retry with 0x prefix
    if !market_id.starts_with("0x") {
        let prefixed = format!("0x{}", market_id);
        let row: Option<MarketInfo> = sqlx::query_as(sql)
            .bind(&prefixed)
            .fetch_optional(pool)
            .await?;

        if row.is_some() {
            return Ok(row);
        }
    }

    // Fallback: search by token_id within clob_token_ids JSON array.
    // Chain listener events use decimal token_ids as market_id.
    let row: Option<MarketInfo> = sqlx::query_as(
        "SELECT slug, question, clob_token_ids, outcomes FROM active_markets WHERE clob_token_ids LIKE '%' || $1 || '%'",
    )
    .bind(market_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
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
