use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Database row for positions table.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Position {
    pub id: Uuid,
    pub market_id: String,
    pub token_id: String,
    pub outcome: String,
    pub size: Decimal,
    pub avg_entry_price: Decimal,
    pub current_price: Option<Decimal>,
    pub unrealized_pnl: Option<Decimal>,
    pub status: Option<String>,
    pub opened_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub realized_pnl: Option<Decimal>,
}
