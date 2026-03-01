use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Database row for copy_orders table.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CopyOrder {
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
    pub placed_at: Option<DateTime<Utc>>,
    pub filled_at: Option<DateTime<Utc>>,
    pub clob_order_id: Option<String>,
}

/// Order status constants.
pub mod order_status {
    pub const PENDING: &str = "pending";
    pub const SUBMITTED: &str = "submitted";
    pub const FILLED: &str = "filled";
    pub const PARTIAL: &str = "partial";
    pub const CANCELLED: &str = "cancelled";
    pub const FAILED: &str = "failed";
}
