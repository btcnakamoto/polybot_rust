use rust_decimal::Decimal;
use uuid::Uuid;

use super::Side;

/// A validated copy-trade signal ready for the execution layer.
#[derive(Debug, Clone)]
pub struct CopySignal {
    /// The whale trade that triggered this signal.
    pub whale_trade_id: Uuid,
    /// Whale's wallet address.
    pub wallet: String,
    /// Market condition ID.
    pub market_id: String,
    /// Token (asset) ID for the specific outcome.
    pub asset_id: String,
    /// Buy or Sell.
    pub side: Side,
    /// Whale's entry price (used as target).
    pub price: Decimal,
    /// Whale's wallet win rate (for Kelly sizing).
    pub whale_win_rate: Decimal,
    /// Whale's Kelly fraction.
    pub whale_kelly: Decimal,
    /// Whale's notional size.
    pub whale_notional: Decimal,
}
