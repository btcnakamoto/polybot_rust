use serde::Serialize;

use crate::models::{WhaleTradeEvent, CopyOrder, Position};

/// Messages broadcast to all connected WebSocket clients.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum WsMessage {
    #[serde(rename = "whale_alert")]
    WhaleAlert(WhaleTradeEvent),

    #[serde(rename = "order_update")]
    OrderUpdate(CopyOrder),

    #[serde(rename = "position_update")]
    PositionUpdate(Position),

    #[serde(rename = "pnl_update")]
    PnlUpdate(PnlSnapshot),

    #[serde(rename = "consensus_alert")]
    ConsensusAlert(ConsensusAlertData),
}

#[derive(Debug, Clone, Serialize)]
pub struct PnlSnapshot {
    pub total_pnl: String,
    pub today_pnl: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConsensusAlertData {
    pub basket_name: String,
    pub category: String,
    pub market_id: String,
    pub direction: String,
    pub consensus_pct: String,
    pub participating_whales: i32,
    pub total_whales: i32,
}
