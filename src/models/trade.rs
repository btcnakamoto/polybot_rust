use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Database row for whale_trades table.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WhaleTrade {
    pub id: Uuid,
    pub whale_id: Option<Uuid>,
    pub market_id: String,
    pub token_id: String,
    pub side: String,
    pub size: Decimal,
    pub price: Decimal,
    pub notional: Decimal,
    pub tx_hash: Option<String>,
    pub traded_at: DateTime<Utc>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Lightweight struct for scoring calculations.
/// Represents the profit outcome of a resolved trade.
#[derive(Debug, Clone)]
pub struct TradeResult {
    pub profit: Decimal,
    pub traded_at: DateTime<Utc>,
}
