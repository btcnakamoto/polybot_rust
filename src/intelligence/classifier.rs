use std::collections::HashSet;
use std::fmt;

use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::models::WhaleTrade;

/// Wallet classification categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Classification {
    /// High-conviction directional trader — worth copying.
    Informed,
    /// Dual-side liquidity provider — do NOT copy.
    MarketMaker,
    /// High-frequency algorithmic trader — do NOT copy.
    Bot,
}

impl Classification {
    pub fn as_str(&self) -> &'static str {
        match self {
            Classification::Informed => "informed",
            Classification::MarketMaker => "market_maker",
            Classification::Bot => "bot",
        }
    }
}

impl fmt::Display for Classification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Classify a wallet based on its trade history.
///
/// Rules:
/// - **MarketMaker**: holds both BUY and SELL in the same market.
/// - **Bot**: >100 trades/month on average.
/// - **Informed**: everything else.
pub fn classify_wallet(trades: &[WhaleTrade]) -> Classification {
    if trades.is_empty() {
        return Classification::Informed;
    }

    // Check for market-maker pattern: dual-side positions in same market
    if is_market_maker(trades) {
        return Classification::MarketMaker;
    }

    // Check for bot pattern: high-frequency trading
    if is_bot(trades) {
        return Classification::Bot;
    }

    Classification::Informed
}

/// Detect market-maker behavior: same wallet has both BUY and SELL
/// in the same market within a short period.
fn is_market_maker(trades: &[WhaleTrade]) -> bool {
    // Group by market_id, track which sides appear
    let mut market_buy: HashSet<String> = HashSet::new();
    let mut market_sell: HashSet<String> = HashSet::new();

    for trade in trades {
        match trade.side.to_uppercase().as_str() {
            "BUY" => {
                market_buy.insert(trade.market_id.clone());
            }
            "SELL" => {
                market_sell.insert(trade.market_id.clone());
            }
            _ => {}
        }
    }

    // If >50% of markets have dual-side activity, likely MM
    let dual_side_count = market_buy.intersection(&market_sell).count();
    let total_markets = market_buy.union(&market_sell).count();

    if total_markets == 0 {
        return false;
    }

    let dual_ratio = Decimal::from(dual_side_count as i64)
        / Decimal::from(total_markets as i64);

    dual_ratio > Decimal::new(50, 2) // >50% markets with dual-side
}

/// Detect bot behavior: average >100 trades per month.
fn is_bot(trades: &[WhaleTrade]) -> bool {
    if trades.len() < 10 {
        return false;
    }

    let oldest = trades
        .iter()
        .map(|t| t.traded_at)
        .min()
        .unwrap_or_else(Utc::now);

    let newest = trades
        .iter()
        .map(|t| t.traded_at)
        .max()
        .unwrap_or_else(Utc::now);

    let span_days = (newest - oldest).num_days().max(1);
    let months = Decimal::from(span_days) / Decimal::from(30);

    if months.is_zero() {
        return trades.len() > 100;
    }

    let trades_per_month = Decimal::from(trades.len() as i64) / months;

    trades_per_month > Decimal::from(100)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use rust_decimal::Decimal;
    use uuid::Uuid;

    fn make_trade(market: &str, side: &str, days_ago: i64) -> WhaleTrade {
        WhaleTrade {
            id: Uuid::new_v4(),
            whale_id: Some(Uuid::new_v4()),
            market_id: market.to_string(),
            token_id: "token_1".to_string(),
            side: side.to_string(),
            size: Decimal::from(100),
            price: Decimal::new(50, 2),
            notional: Decimal::from(50),
            tx_hash: None,
            traded_at: Utc::now() - Duration::days(days_ago),
            created_at: Some(Utc::now()),
        }
    }

    #[test]
    fn test_classify_informed() {
        // 5 BUY trades across different markets over 6 months
        let trades: Vec<WhaleTrade> = (0..5)
            .map(|i| make_trade(&format!("market_{i}"), "BUY", i * 30))
            .collect();

        assert_eq!(classify_wallet(&trades), Classification::Informed);
    }

    #[test]
    fn test_classify_market_maker() {
        // Same market, both BUY and SELL across all markets
        let trades = vec![
            make_trade("market_A", "BUY", 10),
            make_trade("market_A", "SELL", 9),
            make_trade("market_B", "BUY", 8),
            make_trade("market_B", "SELL", 7),
        ];

        assert_eq!(classify_wallet(&trades), Classification::MarketMaker);
    }

    #[test]
    fn test_classify_bot() {
        // 200 trades in a single month = bot
        let trades: Vec<WhaleTrade> = (0..200)
            .map(|_| make_trade("market_X", "BUY", 0))
            .collect();

        assert_eq!(classify_wallet(&trades), Classification::Bot);
    }

    #[test]
    fn test_classify_empty() {
        assert_eq!(classify_wallet(&[]), Classification::Informed);
    }
}
