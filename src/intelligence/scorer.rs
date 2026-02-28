use rust_decimal::Decimal;
use rust_decimal::MathematicalOps;
use serde::{Deserialize, Serialize};

use crate::models::TradeResult;

/// Aggregated scoring output for a wallet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletScore {
    pub sharpe_ratio: Decimal,
    pub win_rate: Decimal,
    pub kelly_fraction: Decimal,
    pub expected_value: Decimal,
    pub total_trades: i32,
    pub total_pnl: Decimal,
    pub is_decaying: bool,
}

/// Compute all scoring metrics for a wallet given its trade history.
pub fn score_wallet(trades: &[TradeResult]) -> WalletScore {
    let total_trades = trades.len() as i32;
    let total_pnl = trades.iter().map(|t| t.profit).sum::<Decimal>();

    let returns: Vec<Decimal> = trades.iter().map(|t| t.profit).collect();

    let sr = sharpe_ratio(&returns);
    let wr = win_rate(trades);
    let ev = expected_value(trades);
    let kf = kelly_fraction(wr, avg_odds(trades));
    let decaying = is_decaying(trades);

    WalletScore {
        sharpe_ratio: sr,
        win_rate: wr,
        kelly_fraction: kf,
        expected_value: ev,
        total_trades,
        total_pnl,
        is_decaying: decaying,
    }
}

// ---------------------------------------------------------------------------
// Metric 1: Sharpe Ratio
// ---------------------------------------------------------------------------

/// Risk-adjusted return: mean(returns) / stddev(returns).
/// Returns Decimal::ZERO if insufficient data.
pub fn sharpe_ratio(returns: &[Decimal]) -> Decimal {
    if returns.len() < 2 {
        return Decimal::ZERO;
    }

    let n = Decimal::from(returns.len() as i64);
    let mean = returns.iter().copied().sum::<Decimal>() / n;

    let variance = returns
        .iter()
        .map(|r| {
            let diff = *r - mean;
            diff * diff
        })
        .sum::<Decimal>()
        / n;

    let std_dev = variance.sqrt().unwrap_or(Decimal::ONE);

    if std_dev.is_zero() {
        return Decimal::ZERO;
    }

    mean / std_dev
}

// ---------------------------------------------------------------------------
// Metric 2: Kelly Fraction
// ---------------------------------------------------------------------------

/// Optimal bet fraction: f = (p * b - q) / b
/// where p = win_rate, q = 1-p, b = avg win/loss ratio (odds).
pub fn kelly_fraction(win_rate: Decimal, avg_odds: Decimal) -> Decimal {
    if avg_odds.is_zero() || win_rate.is_zero() {
        return Decimal::ZERO;
    }

    let q = Decimal::ONE - win_rate;
    let f = (win_rate * avg_odds - q) / avg_odds;

    // Kelly fraction should be non-negative
    f.max(Decimal::ZERO)
}

/// Calculate average odds (avg_win / avg_loss) from trade results.
fn avg_odds(trades: &[TradeResult]) -> Decimal {
    let wins: Vec<&TradeResult> = trades.iter().filter(|t| t.profit > Decimal::ZERO).collect();
    let losses: Vec<&TradeResult> = trades.iter().filter(|t| t.profit < Decimal::ZERO).collect();

    if wins.is_empty() || losses.is_empty() {
        return Decimal::ONE;
    }

    let avg_win = wins.iter().map(|t| t.profit).sum::<Decimal>() / Decimal::from(wins.len() as i64);
    let avg_loss = losses
        .iter()
        .map(|t| t.profit.abs())
        .sum::<Decimal>()
        / Decimal::from(losses.len() as i64);

    if avg_loss.is_zero() {
        return Decimal::ONE;
    }

    avg_win / avg_loss
}

// ---------------------------------------------------------------------------
// Metric 3: Rolling Win Rate + Decay Detection
// ---------------------------------------------------------------------------

/// Overall win rate.
pub fn win_rate(trades: &[TradeResult]) -> Decimal {
    rolling_win_rate(trades, trades.len())
}

/// Rolling win rate over the last `window` trades.
pub fn rolling_win_rate(trades: &[TradeResult], window: usize) -> Decimal {
    if trades.is_empty() {
        return Decimal::ZERO;
    }

    let start = trades.len().saturating_sub(window);
    let recent = &trades[start..];
    let wins = recent.iter().filter(|t| t.profit > Decimal::ZERO).count();

    Decimal::from(wins as i64) / Decimal::from(recent.len() as i64)
}

/// Detect performance decay:
/// - 30-trade rolling WR < 55%, OR
/// - 30-trade rolling WR < 80% of all-time WR
pub fn is_decaying(trades: &[TradeResult]) -> bool {
    if trades.len() < 30 {
        return false;
    }

    let alltime_wr = rolling_win_rate(trades, trades.len());
    let recent_wr = rolling_win_rate(trades, 30);

    let threshold_absolute = Decimal::new(55, 2); // 0.55
    let threshold_relative = alltime_wr * Decimal::new(80, 2) / Decimal::ONE_HUNDRED;

    recent_wr < threshold_absolute || recent_wr < threshold_relative
}

// ---------------------------------------------------------------------------
// Metric 4: Expected Value
// ---------------------------------------------------------------------------

/// Average expected profit per trade.
pub fn expected_value(trades: &[TradeResult]) -> Decimal {
    if trades.is_empty() {
        return Decimal::ZERO;
    }

    let wins: Vec<&TradeResult> = trades.iter().filter(|t| t.profit > Decimal::ZERO).collect();
    let losses: Vec<&TradeResult> = trades.iter().filter(|t| t.profit <= Decimal::ZERO).collect();

    if wins.is_empty() {
        // All losses or break-even
        return trades.iter().map(|t| t.profit).sum::<Decimal>() / Decimal::from(trades.len() as i64);
    }

    let wr = Decimal::from(wins.len() as i64) / Decimal::from(trades.len() as i64);
    let avg_win = wins.iter().map(|t| t.profit).sum::<Decimal>() / Decimal::from(wins.len() as i64);

    if losses.is_empty() {
        return wr * avg_win;
    }

    let avg_loss = losses
        .iter()
        .map(|t| t.profit.abs())
        .sum::<Decimal>()
        / Decimal::from(losses.len() as i64);

    wr * avg_win - (Decimal::ONE - wr) * avg_loss
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_trades(profits: &[i64]) -> Vec<TradeResult> {
        profits
            .iter()
            .map(|&p| TradeResult {
                profit: Decimal::from(p),
                traded_at: Utc::now(),
            })
            .collect()
    }

    #[test]
    fn test_win_rate_basic() {
        let trades = make_trades(&[100, -50, 200, -30, 150]);
        let wr = win_rate(&trades);
        // 3 wins / 5 trades = 0.60
        assert_eq!(wr, Decimal::new(6, 1));
    }

    #[test]
    fn test_win_rate_empty() {
        assert_eq!(win_rate(&[]), Decimal::ZERO);
    }

    #[test]
    fn test_sharpe_ratio_positive() {
        let returns = vec![
            Decimal::from(10),
            Decimal::from(20),
            Decimal::from(15),
            Decimal::from(25),
        ];
        let sr = sharpe_ratio(&returns);
        assert!(sr > Decimal::ZERO, "Sharpe should be positive for all-positive returns");
    }

    #[test]
    fn test_sharpe_ratio_insufficient_data() {
        let returns = vec![Decimal::from(10)];
        assert_eq!(sharpe_ratio(&returns), Decimal::ZERO);
    }

    #[test]
    fn test_kelly_fraction_positive_edge() {
        // 60% win rate, 1.5:1 odds
        let kf = kelly_fraction(
            Decimal::new(60, 2),  // 0.60
            Decimal::new(15, 1),  // 1.5
        );
        // f = (0.6 * 1.5 - 0.4) / 1.5 = (0.9 - 0.4) / 1.5 = 0.333...
        assert!(kf > Decimal::ZERO);
        assert!(kf < Decimal::ONE);
    }

    #[test]
    fn test_kelly_fraction_no_edge() {
        // 40% win rate, 1:1 odds → negative Kelly → clamped to 0
        let kf = kelly_fraction(
            Decimal::new(40, 2),
            Decimal::ONE,
        );
        assert_eq!(kf, Decimal::ZERO);
    }

    #[test]
    fn test_expected_value_positive() {
        let trades = make_trades(&[100, -50, 200, -30, 150]);
        let ev = expected_value(&trades);
        // Positive overall → EV should be positive
        assert!(ev > Decimal::ZERO);
    }

    #[test]
    fn test_is_decaying_not_enough_data() {
        let trades = make_trades(&[100, -50, 200]);
        assert!(!is_decaying(&trades), "Should not flag decay with < 30 trades");
    }

    #[test]
    fn test_is_decaying_detected() {
        // 50 winning trades, then 30 losing trades
        let mut profits = vec![100i64; 50];
        profits.extend(vec![-100i64; 30]);
        let trades = make_trades(&profits);
        assert!(is_decaying(&trades), "Should detect decay when recent WR drops");
    }

    #[test]
    fn test_score_wallet_integration() {
        let trades = make_trades(&[100, -50, 200, -30, 150, 80, -20, 300]);
        let score = score_wallet(&trades);
        assert!(score.sharpe_ratio != Decimal::ZERO);
        assert!(score.win_rate > Decimal::ZERO);
        assert_eq!(score.total_trades, 8);
        assert!(!score.is_decaying);
    }
}
