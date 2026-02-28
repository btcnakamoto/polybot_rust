use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::CopyOrder;

/// Insert a new copy order.
#[allow(clippy::too_many_arguments)]
pub async fn insert_order(
    pool: &PgPool,
    whale_trade_id: Uuid,
    market_id: &str,
    token_id: &str,
    side: &str,
    size: Decimal,
    target_price: Decimal,
    strategy: &str,
) -> anyhow::Result<CopyOrder> {
    let order = sqlx::query_as::<_, CopyOrder>(
        r#"
        INSERT INTO copy_orders (whale_trade_id, market_id, token_id, side, size, target_price, strategy)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *
        "#,
    )
    .bind(whale_trade_id)
    .bind(market_id)
    .bind(token_id)
    .bind(side)
    .bind(size)
    .bind(target_price)
    .bind(strategy)
    .fetch_one(pool)
    .await?;

    Ok(order)
}

/// Mark an order as filled with actual fill price.
pub async fn fill_order(
    pool: &PgPool,
    order_id: Uuid,
    fill_price: Decimal,
    slippage: Decimal,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE copy_orders
        SET status = 'filled', fill_price = $2, slippage = $3, filled_at = $4
        WHERE id = $1
        "#,
    )
    .bind(order_id)
    .bind(fill_price)
    .bind(slippage)
    .bind(Utc::now())
    .execute(pool)
    .await?;

    Ok(())
}

/// Mark an order as failed with error message.
pub async fn fail_order(
    pool: &PgPool,
    order_id: Uuid,
    error_message: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE copy_orders SET status = 'failed', error_message = $2 WHERE id = $1",
    )
    .bind(order_id)
    .bind(error_message)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get pending orders.
pub async fn get_pending_orders(pool: &PgPool) -> anyhow::Result<Vec<CopyOrder>> {
    let orders = sqlx::query_as::<_, CopyOrder>(
        "SELECT * FROM copy_orders WHERE status = 'pending' ORDER BY placed_at ASC",
    )
    .fetch_all(pool)
    .await?;

    Ok(orders)
}
