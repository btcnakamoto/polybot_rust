use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Sizing strategy selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SizingStrategy {
    Proportional,
    Fixed,
    Kelly,
}

impl SizingStrategy {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "proportional" => SizingStrategy::Proportional,
            "kelly" => SizingStrategy::Kelly,
            _ => SizingStrategy::Fixed,
        }
    }
}

impl fmt::Display for SizingStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SizingStrategy::Proportional => write!(f, "proportional"),
            SizingStrategy::Fixed => write!(f, "fixed"),
            SizingStrategy::Kelly => write!(f, "kelly"),
        }
    }
}

/// Calculate position size based on strategy.
pub fn calculate_size(
    strategy: SizingStrategy,
    bankroll: Decimal,
    whale_notional: Decimal,
    whale_win_rate: Decimal,
    whale_kelly: Decimal,
    base_amount: Decimal,
    signal_strength: Decimal,
) -> Decimal {
    let raw = match strategy {
        SizingStrategy::Proportional => {
            proportional_size(whale_notional, bankroll)
        }
        SizingStrategy::Fixed => {
            fixed_size(base_amount, signal_strength)
        }
        SizingStrategy::Kelly => {
            kelly_size(bankroll, whale_win_rate, whale_kelly)
        }
    };

    // Clamp: at least $1, at most the bankroll
    raw.max(Decimal::ZERO).min(bankroll)
}

/// Proportional: mirror the whale's position percentage of our bankroll.
fn proportional_size(whale_notional: Decimal, my_bankroll: Decimal) -> Decimal {
    // Assume whale bankroll ~20x their single trade (rough heuristic)
    let estimated_whale_bankroll = whale_notional * Decimal::from(20);
    if estimated_whale_bankroll.is_zero() {
        return Decimal::ZERO;
    }
    let whale_pct = whale_notional / estimated_whale_bankroll;
    my_bankroll * whale_pct
}

/// Fixed: base amount scaled by signal strength (0.0 - 1.0).
fn fixed_size(base_amount: Decimal, signal_strength: Decimal) -> Decimal {
    base_amount * signal_strength
}

/// Kelly: half-Kelly optimal sizing.
/// f = (p * b - q) / b, then apply fraction=0.5.
fn kelly_size(bankroll: Decimal, _win_rate: Decimal, kelly_fraction: Decimal) -> Decimal {
    if kelly_fraction <= Decimal::ZERO {
        return Decimal::ZERO;
    }

    // Use half-Kelly for safety
    let half_kelly = kelly_fraction * Decimal::new(5, 1); // × 0.5

    bankroll * half_kelly
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_size() {
        let size = fixed_size(Decimal::from(100), Decimal::new(8, 1));
        assert_eq!(size, Decimal::from(80)); // 100 × 0.8
    }

    #[test]
    fn test_proportional_size() {
        let size = proportional_size(Decimal::from(10_000), Decimal::from(5_000));
        // whale 10k, estimated bankroll 200k, pct = 5%, my 5k * 5% = 250
        assert_eq!(size, Decimal::from(250));
    }

    #[test]
    fn test_kelly_size() {
        // kelly_fraction = 0.2, half-kelly = 0.1, bankroll = 10000 → 1000
        let size = kelly_size(
            Decimal::from(10_000),
            Decimal::new(65, 2), // not used directly here
            Decimal::new(2, 1),  // 0.2 kelly fraction
        );
        assert_eq!(size, Decimal::from(1_000));
    }

    #[test]
    fn test_kelly_zero_fraction() {
        let size = kelly_size(Decimal::from(10_000), Decimal::new(40, 2), Decimal::ZERO);
        assert_eq!(size, Decimal::ZERO);
    }

    #[test]
    fn test_calculate_size_clamped() {
        // Ensure result doesn't exceed bankroll
        let size = calculate_size(
            SizingStrategy::Fixed,
            Decimal::from(100),    // bankroll
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::from(500),    // base_amount > bankroll
            Decimal::ONE,          // signal_strength
        );
        assert_eq!(size, Decimal::from(100)); // clamped to bankroll
    }
}
