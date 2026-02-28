pub mod order;
pub mod position;
pub mod signal;
pub mod trade;
pub mod whale;

pub use order::CopyOrder;
pub use position::Position;
pub use signal::CopySignal;
pub use trade::{TradeResult, WhaleTrade};
pub use whale::Whale;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// Side
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    pub fn from_api_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "BUY" | "0" => Some(Side::Buy),
            "SELL" | "1" => Some(Side::Sell),
            _ => None,
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Side::Buy => write!(f, "BUY"),
            Side::Sell => write!(f, "SELL"),
        }
    }
}

// ---------------------------------------------------------------------------
// WhaleTradeEvent — core pipeline message
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhaleTradeEvent {
    pub wallet: String,
    pub market_id: String,
    pub asset_id: String,
    pub side: Side,
    pub size: Decimal,
    pub price: Decimal,
    pub notional: Decimal,
    pub timestamp: DateTime<Utc>,
}

impl fmt::Display for WhaleTradeEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Trade: wallet={} market={} side={} size={} price={} notional={}",
            &self.wallet[..8.min(self.wallet.len())],
            &self.market_id[..8.min(self.market_id.len())],
            self.side,
            self.size,
            self.price,
            self.notional,
        )
    }
}
