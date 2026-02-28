use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::Position;

/// Open a new position or add to an existing one in the same market/token.
pub async fn upsert_position(
    pool: &PgPool,
    market_id: &str,
    token_id: &str,
    outcome: &str,
    size: Decimal,
    entry_price: Decimal,
) -> anyhow::Result<Position> {
    // Try to find an existing open position for this token
    let existing = sqlx::query_as::<_, Position>(
        "SELECT * FROM positions WHERE token_id = $1 AND status = 'open' LIMIT 1",
    )
    .bind(token_id)
    .fetch_optional(pool)
    .await?;

    match existing {
        Some(pos) => {
            // Update existing: weighted average entry price
            let new_size = pos.size + size;
            let new_avg = (pos.avg_entry_price * pos.size + entry_price * size) / new_size;

            let updated = sqlx::query_as::<_, Position>(
                r#"
                UPDATE positions
                SET size = $2, avg_entry_price = $3
                WHERE id = $1
                RETURNING *
                "#,
            )
            .bind(pos.id)
            .bind(new_size)
            .bind(new_avg)
            .fetch_one(pool)
            .await?;

            Ok(updated)
        }
        None => {
            // Create new position
            let pos = sqlx::query_as::<_, Position>(
                r#"
                INSERT INTO positions (market_id, token_id, outcome, size, avg_entry_price)
                VALUES ($1, $2, $3, $4, $5)
                RETURNING *
                "#,
            )
            .bind(market_id)
            .bind(token_id)
            .bind(outcome)
            .bind(size)
            .bind(entry_price)
            .fetch_one(pool)
            .await?;

            Ok(pos)
        }
    }
}

/// Get all open positions.
pub async fn get_open_positions(pool: &PgPool) -> anyhow::Result<Vec<Position>> {
    let positions = sqlx::query_as::<_, Position>(
        "SELECT * FROM positions WHERE status = 'open' ORDER BY opened_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(positions)
}

/// Count open positions.
pub async fn count_open_positions(pool: &PgPool) -> anyhow::Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM positions WHERE status = 'open'",
    )
    .fetch_one(pool)
    .await?;

    Ok(row.0)
}

/// Get all open positions for a specific market.
pub async fn get_positions_for_market(pool: &PgPool, market_id: &str) -> anyhow::Result<Vec<Position>> {
    let positions = sqlx::query_as::<_, Position>(
        "SELECT * FROM positions WHERE market_id = $1 AND status = 'open'",
    )
    .bind(market_id)
    .fetch_all(pool)
    .await?;

    Ok(positions)
}

/// Close a position with realized PnL.
pub async fn close_position(pool: &PgPool, position_id: uuid::Uuid, realized_pnl: Decimal) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE positions
        SET status = 'closed', realized_pnl = $2, closed_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(position_id)
    .bind(realized_pnl)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get today's realized PnL across all closed positions.
pub async fn get_daily_realized_pnl(pool: &PgPool) -> anyhow::Result<Decimal> {
    let row: (Option<Decimal>,) = sqlx::query_as(
        "SELECT COALESCE(SUM(realized_pnl), 0) FROM positions WHERE closed_at >= CURRENT_DATE",
    )
    .fetch_one(pool)
    .await?;

    Ok(row.0.unwrap_or(Decimal::ZERO))
}
