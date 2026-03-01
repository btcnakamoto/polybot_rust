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

/// Get a single position by ID.
pub async fn get_position_by_id(pool: &PgPool, id: uuid::Uuid) -> anyhow::Result<Option<Position>> {
    let pos = sqlx::query_as::<_, Position>(
        "SELECT * FROM positions WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(pos)
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

/// Get all positions (most recent first), limited to 200.
pub async fn get_all_positions(pool: &PgPool) -> anyhow::Result<Vec<Position>> {
    let positions = sqlx::query_as::<_, Position>(
        "SELECT * FROM positions ORDER BY opened_at DESC LIMIT 200",
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

/// Update the current price and last_price_update timestamp for a position.
pub async fn update_position_price(
    pool: &PgPool,
    position_id: uuid::Uuid,
    current_price: Decimal,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE positions
        SET current_price = $2, last_price_update = NOW()
        WHERE id = $1
        "#,
    )
    .bind(position_id)
    .bind(current_price)
    .execute(pool)
    .await?;

    Ok(())
}

/// Update the current price, unrealized PnL, and last_price_update for a position.
pub async fn update_position_price_and_pnl(
    pool: &PgPool,
    position_id: uuid::Uuid,
    current_price: Decimal,
    unrealized_pnl: Decimal,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE positions
        SET current_price = $2, unrealized_pnl = $3, last_price_update = NOW()
        WHERE id = $1
        "#,
    )
    .bind(position_id)
    .bind(current_price)
    .bind(unrealized_pnl)
    .execute(pool)
    .await?;

    Ok(())
}

/// Mark a position as "exiting" — an exit order has been submitted but not yet filled.
pub async fn mark_position_exiting(
    pool: &PgPool,
    position_id: uuid::Uuid,
    exit_reason: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE positions
        SET status = 'exiting', exit_reason = $2
        WHERE id = $1
        "#,
    )
    .bind(position_id)
    .bind(exit_reason)
    .execute(pool)
    .await?;

    Ok(())
}

/// Find an open/exiting position by token_id.
pub async fn get_position_by_token_id(
    pool: &PgPool,
    token_id: &str,
) -> anyhow::Result<Option<Position>> {
    let pos = sqlx::query_as::<_, Position>(
        "SELECT * FROM positions WHERE token_id = $1 AND status IN ('open', 'exiting') LIMIT 1",
    )
    .bind(token_id)
    .fetch_optional(pool)
    .await?;

    Ok(pos)
}

/// Close a position with realized PnL and an exit reason (stop_loss / take_profit).
pub async fn close_position_with_reason(
    pool: &PgPool,
    position_id: uuid::Uuid,
    realized_pnl: Decimal,
    exit_reason: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE positions
        SET status = 'closed',
            realized_pnl = $2,
            closed_at = NOW(),
            exit_reason = $3,
            exited_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(position_id)
    .bind(realized_pnl)
    .bind(exit_reason)
    .execute(pool)
    .await?;

    Ok(())
}

/// Set stop-loss and take-profit percentages for a position.
pub async fn set_position_sl_tp(
    pool: &PgPool,
    position_id: uuid::Uuid,
    stop_loss_pct: Decimal,
    take_profit_pct: Decimal,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE positions
        SET stop_loss_pct = $2, take_profit_pct = $3
        WHERE id = $1
        "#,
    )
    .bind(position_id)
    .bind(stop_loss_pct)
    .bind(take_profit_pct)
    .execute(pool)
    .await?;

    Ok(())
}
