use std::sync::Arc;

use chrono::Utc;
use polymarket_client_sdk::clob::types::OrderStatusType;
use rust_decimal::Decimal;
use sqlx::PgPool;
use tokio::time::{interval, Duration};

use crate::db::{order_repo, position_repo};
use crate::execution::capital_pool::CapitalPool;
use crate::execution::copy_engine::CopyEngineConfig;
use crate::polymarket::trading::TradingClient;

/// Maximum age of a submitted order before auto-cancellation (5 minutes).
const ORDER_STALE_SECS: i64 = 300;

/// Run the fill poller loop. Periodically checks submitted orders against the
/// CLOB to confirm fills, detect cancellations, and auto-cancel stale orders.
pub async fn run_order_fill_poller(
    pool: PgPool,
    trading_client: Arc<TradingClient>,
    capital_pool: CapitalPool,
    engine_config: CopyEngineConfig,
    poll_interval_secs: u64,
) {
    let mut ticker = interval(Duration::from_secs(poll_interval_secs));
    tracing::info!(
        interval_secs = poll_interval_secs,
        "Order fill poller started"
    );

    loop {
        ticker.tick().await;

        let orders = match order_repo::get_submitted_orders(&pool).await {
            Ok(o) => o,
            Err(e) => {
                tracing::error!(error = %e, "Fill poller: failed to fetch submitted orders");
                continue;
            }
        };

        if orders.is_empty() {
            tracing::debug!("Fill poller: no submitted orders");
            continue;
        }

        tracing::debug!(count = orders.len(), "Fill poller: checking submitted orders");

        for order in &orders {
            let clob_order_id = match &order.clob_order_id {
                Some(id) if !id.is_empty() => id.as_str(),
                _ => {
                    tracing::warn!(
                        order_id = %order.id,
                        "Fill poller: submitted order has no CLOB order ID — cancelling"
                    );
                    let _ = order_repo::cancel_order(&pool, order.id).await;
                    // Release capital using whale_trade_id as the reservation key
                    if let Some(wt_id) = order.whale_trade_id {
                        capital_pool.release(&wt_id).await;
                    }
                    continue;
                }
            };

            // Check if order is stale (older than 5 minutes)
            let is_stale = order
                .placed_at
                .map(|placed| {
                    let age = Utc::now() - placed;
                    age.num_seconds() > ORDER_STALE_SECS
                })
                .unwrap_or(false);

            // Query CLOB for order status
            let clob_status = match trading_client.get_order(clob_order_id).await {
                Ok(resp) => resp,
                Err(e) => {
                    tracing::warn!(
                        order_id = %order.id,
                        clob_order_id = clob_order_id,
                        error = %e,
                        "Fill poller: failed to query CLOB order status"
                    );

                    // If stale and can't query, auto-cancel
                    if is_stale {
                        tracing::warn!(
                            order_id = %order.id,
                            "Fill poller: stale order unreachable — cancelling"
                        );
                        // Try to cancel on CLOB side
                        let _ = trading_client.cancel_order(clob_order_id).await;
                        let _ = order_repo::cancel_order(&pool, order.id).await;
                        if let Some(wt_id) = order.whale_trade_id {
                            capital_pool.release(&wt_id).await;
                        }
                    }
                    continue;
                }
            };

            match clob_status.status {
                OrderStatusType::Matched => {
                    // Fully filled
                    let fill_price = clob_status.price;
                    let slippage = if order.target_price > Decimal::ZERO {
                        ((fill_price - order.target_price) / order.target_price * Decimal::from(100)).abs()
                    } else {
                        Decimal::ZERO
                    };

                    tracing::info!(
                        order_id = %order.id,
                        clob_order_id,
                        fill_price = %fill_price,
                        size_matched = %clob_status.size_matched,
                        "Fill poller: order matched"
                    );

                    // Update order as filled
                    if let Err(e) = order_repo::fill_order(&pool, order.id, fill_price, slippage).await {
                        tracing::error!(error = %e, "Fill poller: failed to mark order filled");
                        continue;
                    }

                    // Confirm capital reservation
                    if let Some(wt_id) = order.whale_trade_id {
                        capital_pool.confirm(&wt_id).await;
                    }

                    // Handle based on strategy type
                    if order.strategy == "exit" {
                        // Exit order filled — close the position
                        handle_exit_fill(&pool, order, fill_price).await;
                    } else {
                        // Entry order filled — create/update position
                        let outcome = match order.side.as_str() {
                            "BUY" => "Yes",
                            _ => "No",
                        };

                        match position_repo::upsert_position(
                            &pool,
                            &order.market_id,
                            &order.token_id,
                            outcome,
                            order.size,
                            fill_price,
                        )
                        .await
                        {
                            Ok(position) => {
                                if let Err(e) = position_repo::set_position_sl_tp(
                                    &pool,
                                    position.id,
                                    engine_config.default_stop_loss_pct,
                                    engine_config.default_take_profit_pct,
                                )
                                .await
                                {
                                    tracing::warn!(error = %e, "Fill poller: failed to set SL/TP");
                                }

                                tracing::info!(
                                    order_id = %order.id,
                                    position_id = %position.id,
                                    "Fill poller: position created/updated from fill"
                                );
                            }
                            Err(e) => {
                                tracing::error!(
                                    error = %e,
                                    order_id = %order.id,
                                    "Fill poller: failed to upsert position"
                                );
                            }
                        }
                    }
                }

                OrderStatusType::Live => {
                    // Still waiting for fill
                    if clob_status.size_matched > Decimal::ZERO {
                        tracing::info!(
                            order_id = %order.id,
                            size_matched = %clob_status.size_matched,
                            original_size = %clob_status.original_size,
                            "Fill poller: partial fill in progress"
                        );
                    }

                    // Auto-cancel if stale
                    if is_stale {
                        tracing::warn!(
                            order_id = %order.id,
                            clob_order_id,
                            "Fill poller: order stale (>5min) — cancelling"
                        );
                        if let Err(e) = trading_client.cancel_order(clob_order_id).await {
                            tracing::error!(error = %e, "Fill poller: failed to cancel stale order on CLOB");
                        }
                        let _ = order_repo::cancel_order(&pool, order.id).await;
                        if let Some(wt_id) = order.whale_trade_id {
                            capital_pool.release(&wt_id).await;
                        }
                    }
                }

                OrderStatusType::Canceled | OrderStatusType::Unmatched => {
                    tracing::info!(
                        order_id = %order.id,
                        clob_order_id,
                        status = ?clob_status.status,
                        "Fill poller: order cancelled/unmatched"
                    );

                    let _ = order_repo::cancel_order(&pool, order.id).await;
                    if let Some(wt_id) = order.whale_trade_id {
                        capital_pool.release(&wt_id).await;
                    }
                }

                other => {
                    tracing::debug!(
                        order_id = %order.id,
                        status = ?other,
                        "Fill poller: unexpected order status"
                    );
                }
            }
        }
    }
}

/// Handle a filled exit order: find the "exiting" position and close it with realized PnL.
async fn handle_exit_fill(
    pool: &PgPool,
    order: &crate::models::CopyOrder,
    fill_price: Decimal,
) {
    // Find the position by token_id that is in "exiting" state
    match position_repo::get_position_by_token_id(pool, &order.token_id).await {
        Ok(Some(pos)) => {
            let realized_pnl = (fill_price - pos.avg_entry_price) * pos.size;
            let reason = pos.exit_reason.as_deref().unwrap_or("exit");

            if let Err(e) = position_repo::close_position_with_reason(
                pool, pos.id, realized_pnl, reason,
            )
            .await
            {
                tracing::error!(
                    error = %e,
                    position_id = %pos.id,
                    "Fill poller: failed to close position on exit fill"
                );
                return;
            }

            tracing::info!(
                position_id = %pos.id,
                realized_pnl = %realized_pnl,
                exit_reason = reason,
                "Fill poller: position closed from exit fill"
            );
        }
        Ok(None) => {
            tracing::warn!(
                order_id = %order.id,
                token_id = %order.token_id,
                "Fill poller: no exiting position found for exit fill"
            );
        }
        Err(e) => {
            tracing::error!(
                error = %e,
                token_id = %order.token_id,
                "Fill poller: failed to look up position for exit fill"
            );
        }
    }
}
