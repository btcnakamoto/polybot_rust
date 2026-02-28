use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Whale {
    pub id: Uuid,
    pub address: String,
    pub label: Option<String>,
    pub category: Option<String>,
    pub classification: Option<String>,
    pub sharpe_ratio: Option<Decimal>,
    pub win_rate: Option<Decimal>,
    pub total_trades: Option<i32>,
    pub total_pnl: Option<Decimal>,
    pub kelly_fraction: Option<Decimal>,
    pub expected_value: Option<Decimal>,
    pub is_active: Option<bool>,
    pub last_trade_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
