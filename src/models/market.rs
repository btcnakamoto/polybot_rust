use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Database row for market_outcomes table.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MarketOutcome {
    pub id: Uuid,
    pub market_id: String,
    pub token_id: Option<String>,
    pub outcome: String,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
