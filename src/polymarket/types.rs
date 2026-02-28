use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Market / Token (Data API)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiToken {
    pub token_id: String,
    pub outcome: String,
    #[serde(default)]
    pub price: Option<Decimal>,
    #[serde(default)]
    pub winner: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiMarket {
    pub condition_id: String,
    pub question: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tokens: Vec<ApiToken>,
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub closed: Option<bool>,
    #[serde(default)]
    pub end_date_iso: Option<String>,
}

// ---------------------------------------------------------------------------
// Trade (Data API — REST)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiTrade {
    pub id: Option<String>,
    pub taker_order_id: Option<String>,
    pub market: Option<String>,
    pub asset_id: Option<String>,
    pub side: Option<String>,
    pub size: Option<Decimal>,
    pub price: Option<Decimal>,
    pub maker_address: Option<String>,
    pub taker_address: Option<String>,
    pub timestamp: Option<String>,
    #[serde(default)]
    pub transaction_hash: Option<String>,
}

// ---------------------------------------------------------------------------
// Trade (WebSocket)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsTrade {
    pub id: Option<String>,
    pub taker_order_id: Option<String>,
    pub market: Option<String>,
    pub asset_id: Option<String>,
    pub side: Option<String>,
    pub size: Option<String>,
    pub price: Option<String>,
    pub maker_address: Option<String>,
    pub taker_address: Option<String>,
    pub timestamp: Option<String>,
    #[serde(default)]
    pub transaction_hash: Option<String>,
}

// ---------------------------------------------------------------------------
// WebSocket subscribe message
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct WsSubscribe {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub channel: String,
    pub assets_id: String,
}

impl WsSubscribe {
    pub fn market_trades(asset_id: &str) -> Self {
        Self {
            msg_type: "subscribe".into(),
            channel: "market".into(),
            assets_id: asset_id.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Order Book (CLOB API)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiOrderBookLevel {
    pub price: Decimal,
    pub size: Decimal,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiOrderBook {
    pub market: Option<String>,
    pub asset_id: Option<String>,
    #[serde(default)]
    pub bids: Vec<ApiOrderBookLevel>,
    #[serde(default)]
    pub asks: Vec<ApiOrderBookLevel>,
    pub hash: Option<String>,
    pub timestamp: Option<String>,
}
