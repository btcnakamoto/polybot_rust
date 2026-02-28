use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// A whale basket — a group of whales categorised by topic.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WhaleBasket {
    pub id: Uuid,
    pub name: String,
    pub category: String,
    pub consensus_threshold: Decimal,
    pub time_window_hours: i32,
    pub min_wallets: i32,
    pub max_wallets: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Association between a basket and a whale.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BasketWallet {
    pub id: Uuid,
    pub basket_id: Uuid,
    pub whale_id: Uuid,
    pub added_at: DateTime<Utc>,
}

/// A recorded consensus signal (audit log).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ConsensusSignal {
    pub id: Uuid,
    pub basket_id: Uuid,
    pub market_id: String,
    pub direction: String,
    pub consensus_pct: Decimal,
    pub participating_whales: i32,
    pub total_whales: i32,
    pub triggered_at: DateTime<Utc>,
}

/// Basket category taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BasketCategory {
    Politics,
    Crypto,
    Sports,
}

impl BasketCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            BasketCategory::Politics => "politics",
            BasketCategory::Crypto => "crypto",
            BasketCategory::Sports => "sports",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "politics" => Some(BasketCategory::Politics),
            "crypto" => Some(BasketCategory::Crypto),
            "sports" => Some(BasketCategory::Sports),
            _ => None,
        }
    }
}

impl std::fmt::Display for BasketCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
