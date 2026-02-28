use rust_decimal::Decimal;
use thiserror::Error;

use crate::polymarket::ClobClient;

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
}

/// Result of an executed order.
#[derive(Debug, Clone)]
pub struct OrderResult {
    pub fill_price: Decimal,
    pub slippage: Decimal,
    pub success: bool,
}

/// Executes orders against the Polymarket CLOB.
///
/// In Phase 3, this validates against the orderbook and records the intent.
/// Actual on-chain execution (EIP-712 signing + order placement) will be
/// wired in when we add wallet/private-key support.
pub struct OrderExecutor {
    clob_client: Option<ClobClient>,
    risk_limits: RiskLimits,
}

impl OrderExecutor {
    pub fn new(clob_client: Option<ClobClient>, risk_limits: RiskLimits) -> Self {
        Self {
            clob_client,
            risk_limits,
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
        let client = match &self.clob_client {
            Some(c) => c,
            None => {
                // Dry-run mode: no CLOB client configured
                tracing::info!(
                    token_id,
                    side,
                    size = %size,
                    target_price = %target_price,
                    "[DRY-RUN] Would place limit order"
                );
                return Ok(OrderResult {
                    fill_price: target_price,
                    slippage: Decimal::ZERO,
                    success: true,
                });
            }
        };

        // 1. Fetch orderbook
        let book = client
            .get_order_book(token_id)
            .await
            .map_err(|e| ExecutionError::ClobError(e.to_string()))?;

        // 2. Determine current price from orderbook
        let current_price = match side.to_uppercase().as_str() {
            "BUY" => {
                // Best ask (lowest sell offer)
                book.asks
                    .first()
                    .map(|l| l.price)
                    .ok_or_else(|| ExecutionError::EmptyOrderbook(token_id.to_string()))?
            }
            "SELL" => {
                // Best bid (highest buy offer)
                book.bids
                    .first()
                    .map(|l| l.price)
                    .ok_or_else(|| ExecutionError::EmptyOrderbook(token_id.to_string()))?
            }
            _ => target_price,
        };

        // 3. Slippage check
        let slippage = check_slippage(target_price, current_price, &self.risk_limits)?;

        tracing::info!(
            token_id,
            side,
            size = %size,
            target_price = %target_price,
            current_price = %current_price,
            slippage = %slippage,
            "Placing limit order on CLOB"
        );

        // 4. Place limit order
        // NOTE: Actual order signing (EIP-712) and placement will be added
        // when wallet private key support is implemented. For now, we log
        // the validated order intent.
        tracing::warn!(
            "[PHASE 3] Order validated but on-chain submission requires wallet signing (Phase 3+)"
        );

        Ok(OrderResult {
            fill_price: current_price,
            slippage,
            success: true,
        })
    }
}
