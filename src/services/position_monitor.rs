use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rust_decimal::Decimal;
use sqlx::PgPool;
use tokio::time::{interval, Duration};

use crate::db::position_repo;
use crate::polymarket::clob_client::ClobClient;
use crate::polymarket::trading::TradingClient;
use crate::services::notifier::Notifier;

/// Run the position monitor loop. Periodically checks open positions,
/// fetches current prices from the CLOB orderbook, and triggers stop-loss
/// or take-profit exits when thresholds are breached.
pub async fn run_position_monitor(
    pool: PgPool,
    clob_client: ClobClient,
    trading_client: Option<Arc<TradingClient>>,
    dry_run: bool,
    pause_flag: Arc<AtomicBool>,
    interval_secs: u64,
    notifier: Option<Arc<Notifier>>,
) {
    let mut ticker = interval(Duration::from_secs(interval_secs));

    loop {
        ticker.tick().await;

        // Respect pause flag
        if pause_flag.load(Ordering::Relaxed) {
            tracing::debug!("Position monitor paused");
            continue;
        }

        let positions = match position_repo::get_open_positions(&pool).await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!(error = %e, "Position monitor: failed to fetch open positions");
                continue;
            }
        };

        if positions.is_empty() {
            tracing::debug!("Position monitor: no open positions");
            continue;
        }

        for pos in &positions {
            // Fetch current best price from orderbook
            let current_price = match clob_client.get_order_book(&pos.token_id).await {
                Ok(book) => {
                    // For a position we hold, the exit price is the best bid
                    // (we'd be selling into bids)
                    match book.bids.first() {
                        Some(level) => level.price,
                        None => {
                            tracing::debug!(
                                token_id = %pos.token_id,
                                "No bids in orderbook — skipping price update"
                            );
                            continue;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        token_id = %pos.token_id,
                        "Failed to fetch orderbook for position"
                    );
                    continue;
                }
            };

            // Update current price in DB
            if let Err(e) = position_repo::update_position_price(&pool, pos.id, current_price).await {
                tracing::warn!(error = %e, "Failed to update position price");
            }

            // Calculate PnL percentage
            if pos.avg_entry_price == Decimal::ZERO {
                continue;
            }
            let pnl_pct =
                (current_price - pos.avg_entry_price) / pos.avg_entry_price * Decimal::from(100);

            let stop_loss = pos.stop_loss_pct.unwrap_or(Decimal::new(1500, 2)); // 15.00
            let take_profit = pos.take_profit_pct.unwrap_or(Decimal::new(5000, 2)); // 50.00

            let exit_reason = if pnl_pct <= -stop_loss {
                Some("stop_loss")
            } else if pnl_pct >= take_profit {
                Some("take_profit")
            } else {
                None
            };

            let Some(reason) = exit_reason else {
                tracing::debug!(
                    token_id = %pos.token_id,
                    entry = %pos.avg_entry_price,
                    current = %current_price,
                    pnl_pct = %pnl_pct,
                    "Position within SL/TP bounds"
                );
                continue;
            };

            tracing::info!(
                token_id = %pos.token_id,
                entry = %pos.avg_entry_price,
                current = %current_price,
                pnl_pct = %pnl_pct,
                reason = reason,
                "SL/TP triggered — exiting position"
            );

            // Execute sell order
            if !dry_run {
                if let Some(ref tc) = trading_client {
                    match tc
                        .place_limit_order(&pos.token_id, "SELL", pos.size, current_price)
                        .await
                    {
                        Ok(resp) => {
                            if resp.success {
                                tracing::info!(
                                    token_id = %pos.token_id,
                                    order_id = %resp.order_id,
                                    "Exit order placed successfully"
                                );
                            } else {
                                let msg = resp.error_msg.unwrap_or_default();
                                tracing::error!(
                                    token_id = %pos.token_id,
                                    error = %msg,
                                    "Exit order rejected"
                                );
                                continue;
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                error = %e,
                                token_id = %pos.token_id,
                                "Failed to place exit order"
                            );
                            continue;
                        }
                    }
                } else {
                    tracing::warn!(
                        token_id = %pos.token_id,
                        "No trading client — cannot exit position"
                    );
                    continue;
                }
            } else {
                tracing::info!(
                    token_id = %pos.token_id,
                    size = %pos.size,
                    price = %current_price,
                    reason = reason,
                    "[DRY-RUN] Would place exit order"
                );
            }

            // Close position in DB
            let realized_pnl = (current_price - pos.avg_entry_price) * pos.size;
            if let Err(e) =
                position_repo::close_position_with_reason(&pool, pos.id, realized_pnl, reason).await
            {
                tracing::error!(error = %e, "Failed to close position in DB");
                continue;
            }

            tracing::info!(
                position_id = %pos.id,
                reason = reason,
                realized_pnl = %realized_pnl,
                "Position closed"
            );

            // Notify
            if let Some(ref n) = notifier {
                let msg = format!(
                    "*Position Exit*\nToken: `{}`\nReason: {}\nEntry: {}\nExit: {}\nPnL: {} USDC\nPnL%: {}%",
                    pos.token_id,
                    reason.replace('_', " "),
                    pos.avg_entry_price,
                    current_price,
                    realized_pnl,
                    pnl_pct.round_dp(2),
                );
                n.send(&msg).await;
            }
        }
    }
}
