use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Configurable risk limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLimits {
    /// Max single position as fraction of bankroll (default 20%).
    pub max_position_pct: Decimal,
    /// Max concurrent open positions (default 10).
    pub max_open_positions: i64,
    /// Max daily loss in USDC (default 500).
    pub max_daily_loss: Decimal,
    /// Min distance from resolution price (0 or 1), default 0.05.
    pub min_spread_to_resolution: Decimal,
    /// Max acceptable slippage percentage (default 3%).
    pub max_slippage_pct: Decimal,
}

impl Default for RiskLimits {
    fn default() -> Self {
        Self {
            max_position_pct: Decimal::new(20, 2),      // 0.20
            max_open_positions: 10,
            max_daily_loss: Decimal::from(500),
            min_spread_to_resolution: Decimal::new(5, 2), // 0.05
            max_slippage_pct: Decimal::new(3, 2),         // 0.03
        }
    }
}

/// Current portfolio state for risk checks.
#[derive(Debug, Clone)]
pub struct PortfolioSnapshot {
    pub bankroll: Decimal,
    pub open_positions: i64,
    pub daily_pnl: Decimal,
}

/// Risk check violation.
#[derive(Debug, Error)]
pub enum RiskViolation {
    #[error("position size {size} exceeds max {max} ({pct}% of bankroll)")]
    PositionTooLarge {
        size: Decimal,
        max: Decimal,
        pct: Decimal,
    },

    #[error("too many open positions: {current}/{max}")]
    TooManyPositions { current: i64, max: i64 },

    #[error("daily loss limit exceeded: PnL {pnl}, limit -{limit}")]
    DailyLossExceeded { pnl: Decimal, limit: Decimal },

    #[error("spread too narrow: distance {distance}, min {min}")]
    SpreadTooNarrow { distance: Decimal, min: Decimal },

    #[error("slippage too high: {actual}% > max {max}%")]
    SlippageTooHigh { actual: Decimal, max: Decimal },
}

/// A pending order to be validated by risk checks.
#[derive(Debug, Clone)]
pub struct PendingOrder {
    pub size: Decimal,
    pub price: Decimal,
}

/// Run all 5 risk checks on a pending order. Returns Ok(()) if all pass.
pub fn check_risk(
    order: &PendingOrder,
    portfolio: &PortfolioSnapshot,
    limits: &RiskLimits,
) -> Result<(), RiskViolation> {
    // 1. Single position size check
    let max_size = portfolio.bankroll * limits.max_position_pct;
    if order.size > max_size {
        return Err(RiskViolation::PositionTooLarge {
            size: order.size,
            max: max_size,
            pct: limits.max_position_pct * Decimal::ONE_HUNDRED,
        });
    }

    // 2. Open position count check
    if portfolio.open_positions >= limits.max_open_positions {
        return Err(RiskViolation::TooManyPositions {
            current: portfolio.open_positions,
            max: limits.max_open_positions,
        });
    }

    // 3. Daily loss check
    if portfolio.daily_pnl < -limits.max_daily_loss {
        return Err(RiskViolation::DailyLossExceeded {
            pnl: portfolio.daily_pnl,
            limit: limits.max_daily_loss,
        });
    }

    // 4. Spread-to-resolution check (price must be >5¢ from 0 or 1)
    let distance = order.price.min(Decimal::ONE - order.price);
    if distance < limits.min_spread_to_resolution {
        return Err(RiskViolation::SpreadTooNarrow {
            distance,
            min: limits.min_spread_to_resolution,
        });
    }

    Ok(())
}

/// Check slippage between target and actual price.
pub fn check_slippage(
    target_price: Decimal,
    actual_price: Decimal,
    limits: &RiskLimits,
) -> Result<Decimal, RiskViolation> {
    if target_price.is_zero() {
        return Ok(Decimal::ZERO);
    }

    let slippage = ((actual_price - target_price) / target_price).abs();

    if slippage > limits.max_slippage_pct {
        return Err(RiskViolation::SlippageTooHigh {
            actual: slippage * Decimal::ONE_HUNDRED,
            max: limits.max_slippage_pct * Decimal::ONE_HUNDRED,
        });
    }

    Ok(slippage)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_portfolio() -> PortfolioSnapshot {
        PortfolioSnapshot {
            bankroll: Decimal::from(10_000),
            open_positions: 0,
            daily_pnl: Decimal::ZERO,
        }
    }

    #[test]
    fn test_risk_check_passes() {
        let order = PendingOrder {
            size: Decimal::from(400),
            price: Decimal::new(45, 2), // 0.45
        };
        let result = check_risk(&order, &default_portfolio(), &RiskLimits::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_position_too_large() {
        let order = PendingOrder {
            size: Decimal::from(2500), // > 20% of 10k = 2000
            price: Decimal::new(50, 2),
        };
        let result = check_risk(&order, &default_portfolio(), &RiskLimits::default());
        assert!(matches!(result, Err(RiskViolation::PositionTooLarge { .. })));
    }

    #[test]
    fn test_too_many_positions() {
        let portfolio = PortfolioSnapshot {
            open_positions: 10,
            ..default_portfolio()
        };
        let order = PendingOrder {
            size: Decimal::from(100),
            price: Decimal::new(50, 2),
        };
        let result = check_risk(&order, &portfolio, &RiskLimits::default());
        assert!(matches!(result, Err(RiskViolation::TooManyPositions { .. })));
    }

    #[test]
    fn test_daily_loss_exceeded() {
        let portfolio = PortfolioSnapshot {
            daily_pnl: Decimal::from(-600), // > -500 limit
            ..default_portfolio()
        };
        let order = PendingOrder {
            size: Decimal::from(100),
            price: Decimal::new(50, 2),
        };
        let result = check_risk(&order, &portfolio, &RiskLimits::default());
        assert!(matches!(result, Err(RiskViolation::DailyLossExceeded { .. })));
    }

    #[test]
    fn test_spread_too_narrow() {
        let order = PendingOrder {
            size: Decimal::from(100),
            price: Decimal::new(97, 2), // 0.97 → distance 0.03 < 0.05
        };
        let result = check_risk(&order, &default_portfolio(), &RiskLimits::default());
        assert!(matches!(result, Err(RiskViolation::SpreadTooNarrow { .. })));
    }

    #[test]
    fn test_slippage_ok() {
        let result = check_slippage(
            Decimal::new(50, 2),
            Decimal::new(51, 2),
            &RiskLimits::default(),
        );
        assert!(result.is_ok());
        let slippage = result.unwrap();
        assert_eq!(slippage, Decimal::new(2, 2)); // 2%
    }

    #[test]
    fn test_slippage_too_high() {
        let result = check_slippage(
            Decimal::new(50, 2),
            Decimal::new(55, 2), // 10% slippage
            &RiskLimits::default(),
        );
        assert!(matches!(result, Err(RiskViolation::SlippageTooHigh { .. })));
    }
}
