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

/// Transition a pending order to submitted with its CLOB order ID.
pub async fn mark_order_submitted(
    pool: &PgPool,
    order_id: Uuid,
    clob_order_id: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE copy_orders SET status = 'submitted', clob_order_id = $2 WHERE id = $1",
    )
    .bind(order_id)
    .bind(clob_order_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get all orders in 'submitted' status (awaiting fill confirmation).
pub async fn get_submitted_orders(pool: &PgPool) -> anyhow::Result<Vec<CopyOrder>> {
    let orders = sqlx::query_as::<_, CopyOrder>(
        "SELECT * FROM copy_orders WHERE status = 'submitted' ORDER BY placed_at ASC",
    )
    .fetch_all(pool)
    .await?;

    Ok(orders)
}

/// Mark an order as cancelled.
pub async fn cancel_order(pool: &PgPool, order_id: Uuid) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE copy_orders SET status = 'cancelled' WHERE id = $1",
    )
    .bind(order_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get all orders (most recent first), limited to 200.
pub async fn get_all_orders(pool: &PgPool) -> anyhow::Result<Vec<CopyOrder>> {
    let orders = sqlx::query_as::<_, CopyOrder>(
        "SELECT * FROM copy_orders ORDER BY placed_at DESC LIMIT 200",
    )
    .fetch_all(pool)
    .await?;

    Ok(orders)
}

/// Enriched order with whale address for dashboard display.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct EnrichedCopyOrder {
    // copy_orders fields
    pub id: Uuid,
    pub whale_trade_id: Option<Uuid>,
    pub market_id: String,
    pub token_id: String,
    pub side: String,
    pub size: Decimal,
    pub target_price: Decimal,
    pub fill_price: Option<Decimal>,
    pub slippage: Option<Decimal>,
    pub status: String,
    pub strategy: String,
    pub error_message: Option<String>,
    pub placed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub filled_at: Option<chrono::DateTime<chrono::Utc>>,
    pub clob_order_id: Option<String>,
    // joined whale info
    pub whale_address: Option<String>,
    pub whale_label: Option<String>,
    pub market_question: Option<String>,
}

/// Get all orders enriched with whale address and market question (most recent first, limit 200).
pub async fn get_all_orders_enriched(pool: &PgPool) -> anyhow::Result<Vec<EnrichedCopyOrder>> {
    let orders = sqlx::query_as::<_, EnrichedCopyOrder>(
        r#"
        SELECT co.*,
               w.address AS whale_address,
               w.label   AS whale_label,
               COALESCE(am1.question, am2.question) AS market_question
        FROM copy_orders co
        LEFT JOIN whale_trades wt ON co.whale_trade_id = wt.id
        LEFT JOIN whales w ON wt.whale_id = w.id
        LEFT JOIN active_markets am1 ON co.market_id = am1.condition_id
        LEFT JOIN active_markets am2 ON am2.clob_token_ids LIKE '%' || co.token_id || '%'
        ORDER BY co.placed_at DESC
        LIMIT 200
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(orders)
}
