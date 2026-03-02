use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgPool;
use tokio::time::{interval, Duration};

use crate::db::{config_repo, market_repo, order_repo, position_repo};
use crate::execution::capital_pool::CapitalPool;
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
    capital_pool: Option<CapitalPool>,
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

        // Read runtime config for trailing stop and time exit
        let mut trailing_stop_pct = Decimal::new(10, 0); // default 10%
        let mut max_hold_days: i64 = 7; // default 7 days
        if let Ok(entries) = config_repo::get_all_config(&pool).await {
            for entry in &entries {
                match entry.key.as_str() {
                    "trailing_stop_pct" => {
                        if let Ok(v) = entry.value.parse() { trailing_stop_pct = v; }
                    }
                    "max_position_hold_days" => {
                        if let Ok(v) = entry.value.parse() { max_hold_days = v; }
                    }
                    _ => {}
                }
            }
        }

        for pos in &positions {
            // Skip positions that already have an exit order in flight
            if pos.status.as_deref() == Some("exiting") {
                tracing::debug!(
                    token_id = %pos.token_id,
                    "Position is exiting — skipping (fill poller will close)"
                );
                continue;
            }

            // Fetch current best price from orderbook
            let current_price = match clob_client.get_order_book(&pos.token_id).await {
                Ok(book) => {
                    // For a position we hold, the exit price is the best (highest) bid.
                    // CLOB API returns bids in ascending order, so use .last() or max.
                    match book.bids.iter().max_by_key(|l| l.price) {
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

            // Compute unrealized PnL and update price + pnl in DB
            let unrealized_pnl = (current_price - pos.avg_entry_price) * pos.size;
            if let Err(e) = position_repo::update_position_price_and_pnl(
                &pool, pos.id, current_price, unrealized_pnl,
            ).await {
                tracing::warn!(error = %e, "Failed to update position price/pnl");
            }

            // Calculate PnL percentage
            if pos.avg_entry_price == Decimal::ZERO {
                continue;
            }
            let pnl_pct =
                (current_price - pos.avg_entry_price) / pos.avg_entry_price * Decimal::from(100);

            let stop_loss = pos.stop_loss_pct.unwrap_or(Decimal::new(1500, 2)); // 15.00
            let take_profit = pos.take_profit_pct.unwrap_or(Decimal::new(2000, 2)); // 20.00

            let exit_reason = if pnl_pct <= -stop_loss {
                Some("stop_loss")
            } else if pnl_pct >= take_profit {
                Some("take_profit")
            } else {
                // Trailing stop: only active when position was AND IS profitable.
                // If price drops below entry, the fixed SL handles it instead.
                let peak = pos.peak_price.unwrap_or(pos.avg_entry_price);
                if peak > pos.avg_entry_price
                    && current_price > pos.avg_entry_price
                    && trailing_stop_pct > Decimal::ZERO
                {
                    let trailing_level =
                        peak * (Decimal::ONE - trailing_stop_pct / Decimal::ONE_HUNDRED);
                    if current_price <= trailing_level {
                        Some("trailing_stop")
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            // Time-based exit: close if held too long
            let exit_reason = exit_reason.or_else(|| {
                if let Some(opened) = pos.opened_at {
                    let hold_days = (Utc::now() - opened).num_days();
                    if max_hold_days > 0 && hold_days >= max_hold_days {
                        return Some("time_exit");
                    }
                }
                None
            });

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

                                // Record exit order in copy_orders and mark as submitted
                                match order_repo::insert_order(
                                    &pool,
                                    // Use a nil UUID since there's no whale_trade_id for exits
                                    uuid::Uuid::nil(),
                                    &pos.market_id,
                                    &pos.token_id,
                                    "SELL",
                                    pos.size,
                                    current_price,
                                    "exit",
                                )
                                .await
                                {
                                    Ok(exit_order) => {
                                        let clob_id = if resp.order_id.is_empty() {
                                            ""
                                        } else {
                                            &resp.order_id
                                        };
                                        if let Err(e) = order_repo::mark_order_submitted(
                                            &pool, exit_order.id, clob_id,
                                        ).await {
                                            tracing::error!(error = %e, "Failed to mark exit order as submitted");
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!(error = %e, "Failed to record exit order in DB");
                                    }
                                }

                                // Mark position as exiting — fill poller will close it
                                if let Err(e) = position_repo::mark_position_exiting(
                                    &pool, pos.id, reason,
                                ).await {
                                    tracing::error!(error = %e, "Failed to mark position as exiting");
                                }
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

                // In dry-run mode, close position immediately (no CLOB order to track)
                let realized_pnl = (current_price - pos.avg_entry_price) * pos.size;
                if let Err(e) =
                    position_repo::close_position_with_reason(&pool, pos.id, realized_pnl, reason).await
                {
                    tracing::error!(error = %e, "Failed to close position in DB");
                    continue;
                }

                // Return capital to the pool (entry cost + realized PnL)
                if let Some(ref cp) = capital_pool {
                    let returned = pos.avg_entry_price * pos.size + realized_pnl;
                    cp.return_capital(returned).await;
                }

                tracing::info!(
                    position_id = %pos.id,
                    reason = reason,
                    realized_pnl = %realized_pnl,
                    "Position closed (dry-run)"
                );

                // Notify
                if let Some(ref n) = notifier {
                    let market_question = market_repo::get_market_question(&pool, &pos.market_id)
                        .await
                        .ok()
                        .flatten();
                    let msg = crate::services::notifier::format_position_exit(
                        market_question.as_deref(),
                        &pos.market_id,
                        reason,
                        pos.avg_entry_price,
                        current_price,
                        realized_pnl,
                        pnl_pct,
                    );
                    n.send(&msg).await;
                }
            }
        }
    }
}
