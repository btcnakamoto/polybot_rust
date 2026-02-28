use rust_decimal::Decimal;
use thiserror::Error;

use crate::polymarket::clob_client::ClobClient;
use crate::polymarket::trading::TradingClient;

use super::risk_manager::{check_slippage, RiskLimits, RiskViolation};

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("risk violation: {0}")]
    RiskViolation(#[from] RiskViolation),

    #[error("CLOB API error: {0}")]
    ClobError(String),

    #[error("orderbook empty for token {0}")]
    EmptyOrderbook(String),

    #[error("no authenticated CLOB client available")]
    NoClient,

    #[error("order rejected by CLOB: {0}")]
    OrderRejected(String),
}

/// Result of an executed order.
#[derive(Debug, Clone)]
pub struct OrderResult {
    pub fill_price: Decimal,
    pub slippage: Decimal,
    pub success: bool,
    /// CLOB order ID returned by the exchange (None for dry-run).
    pub order_id: Option<String>,
}

/// Executes orders against the Polymarket CLOB.
///
/// Supports three modes:
/// - **dry_run=true**: Logs intent, returns simulated success.
/// - **dry_run=false + TradingClient**: Real on-chain order via SDK.
/// - **No TradingClient**: Falls back to dry-run regardless of flag.
pub struct OrderExecutor {
    clob_client: Option<ClobClient>,
    trading_client: Option<TradingClient>,
    risk_limits: RiskLimits,
    dry_run: bool,
}

impl OrderExecutor {
    pub fn new(
        trading_client: Option<TradingClient>,
        clob_client: Option<ClobClient>,
        risk_limits: RiskLimits,
        dry_run: bool,
    ) -> Self {
        Self {
            clob_client,
            trading_client,
            risk_limits,
            dry_run,
        }
    }

    /// Execute a copy-trade order:
    /// 1. Fetch orderbook to get current price
    /// 2. Check slippage vs target
    /// 3. Place limit order (or dry-run log)
    pub async fn execute(
        &self,
        token_id: &str,
        side: &str,
        size: Decimal,
        target_price: Decimal,
    ) -> Result<OrderResult, ExecutionError> {
        // If dry_run or no trading client → simulated execution
        if self.dry_run || self.trading_client.is_none() {
            let mode = if self.trading_client.is_none() {
                "no-wallet"
            } else {
                "dry-run"
            };
            tracing::info!(
                token_id,
                side,
                size = %size,
                target_price = %target_price,
                mode,
                "[DRY-RUN] Would place limit order"
            );
            return Ok(OrderResult {
                fill_price: target_price,
                slippage: Decimal::ZERO,
                success: true,
                order_id: None,
            });
        }

        // --- Live execution path ---

        // 1. Fetch orderbook for slippage validation (use ClobClient if available)
        let current_price = if let Some(client) = &self.clob_client {
            match client.get_order_book(token_id).await {
                Ok(book) => {
                    match side.to_uppercase().as_str() {
                        "BUY" => book
                            .asks
                            .first()
                            .map(|l| l.price)
                            .ok_or_else(|| ExecutionError::EmptyOrderbook(token_id.to_string()))?,
                        "SELL" => book
                            .bids
                            .first()
                            .map(|l| l.price)
                            .ok_or_else(|| ExecutionError::EmptyOrderbook(token_id.to_string()))?,
                        _ => target_price,
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "Failed to fetch orderbook for slippage check, using target price"
                    );
                    target_price
                }
            }
        } else {
            // No ClobClient — skip orderbook slippage, use target price
            target_price
        };

        // 2. Slippage check
        let slippage = check_slippage(target_price, current_price, &self.risk_limits)?;

        tracing::info!(
            token_id,
            side,
            size = %size,
            target_price = %target_price,
            current_price = %current_price,
            slippage = %slippage,
            "Placing live limit order on CLOB"
        );

        // 3. Place real order via SDK
        let trading = self.trading_client.as_ref().expect("checked above");
        let response = trading
            .place_limit_order(token_id, side, size, current_price)
            .await
            .map_err(|e| ExecutionError::ClobError(e.to_string()))?;

        // 4. Check response
        if !response.success {
            let msg = response
                .error_msg
                .unwrap_or_else(|| "unknown CLOB error".into());
            return Err(ExecutionError::OrderRejected(msg));
        }

        let order_id = if response.order_id.is_empty() {
            None
        } else {
            Some(response.order_id.clone())
        };

        tracing::info!(
            order_id = ?order_id,
            fill_price = %current_price,
            slippage = %slippage,
            "Live order placed successfully"
        );

        Ok(OrderResult {
            fill_price: current_price,
            slippage,
            success: true,
            order_id,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dry_run_returns_success() {
        let executor = OrderExecutor::new(None, None, RiskLimits::default(), true);
        let result = executor
            .execute(
                "12345",
                "BUY",
                Decimal::from(50),
                Decimal::new(55, 2), // 0.55
            )
            .await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.success);
        assert_eq!(r.fill_price, Decimal::new(55, 2));
        assert_eq!(r.slippage, Decimal::ZERO);
        assert!(r.order_id.is_none());
    }

    #[tokio::test]
    async fn test_no_trading_client_auto_dry_run() {
        // Even with dry_run=false, missing trading_client forces dry-run
        let executor = OrderExecutor::new(None, None, RiskLimits::default(), false);
        let result = executor
            .execute(
                "12345",
                "SELL",
                Decimal::from(100),
                Decimal::new(40, 2),
            )
            .await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.success);
        assert!(r.order_id.is_none());
    }
}
