use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use metrics::counter;
use rust_decimal::Decimal;
use sqlx::PgPool;
use tokio::sync::mpsc;

use crate::db::{market_repo, order_repo, position_repo};
use crate::models::CopySignal;
use crate::polymarket::balance::BalanceChecker;
use crate::services::notifier::Notifier;

use super::order_executor::OrderExecutor;
use super::position_sizer::{self, SizingStrategy};
use super::risk_manager::{self, PendingOrder, PortfolioSnapshot, RiskLimits};

/// Configuration for the copy engine.
#[derive(Debug, Clone)]
pub struct CopyEngineConfig {
    pub strategy: SizingStrategy,
    pub bankroll: Decimal,
    pub base_amount: Decimal,
    pub risk_limits: RiskLimits,
    pub dry_run: bool,
    pub default_stop_loss_pct: Decimal,
    pub default_take_profit_pct: Decimal,
}

impl Default for CopyEngineConfig {
    fn default() -> Self {
        Self {
            strategy: SizingStrategy::Kelly,
            bankroll: Decimal::from(1_000),
            base_amount: Decimal::from(50),
            risk_limits: RiskLimits::default(),
            dry_run: true,
            default_stop_loss_pct: Decimal::new(1500, 2),  // 15.00%
            default_take_profit_pct: Decimal::new(5000, 2), // 50.00%
        }
    }
}

/// Run the copy engine loop. Receives CopySignals and executes trades.
pub async fn run_copy_engine(
    mut rx: mpsc::Receiver<CopySignal>,
    pool: PgPool,
    executor: OrderExecutor,
    config: CopyEngineConfig,
    notifier: Option<Arc<Notifier>>,
    balance_checker: Option<BalanceChecker>,
    pause_flag: Arc<AtomicBool>,
) {
    tracing::info!(
        strategy = %config.strategy,
        bankroll = %config.bankroll,
        dry_run = config.dry_run,
        "Copy engine started"
    );

    while let Some(signal) = rx.recv().await {
        // Check pause flag
        if pause_flag.load(Ordering::Relaxed) {
            tracing::info!(
                wallet = %signal.wallet,
                market = %signal.market_id,
                "Copy engine paused — skipping signal"
            );
            continue;
        }

        tracing::info!(
            wallet = %signal.wallet,
            market = %signal.market_id,
            side = %signal.side,
            price = %signal.price,
            "Processing copy signal"
        );

        if let Err(e) = process_signal(
            &signal,
            &pool,
            &executor,
            &config,
            notifier.as_deref(),
            balance_checker.as_ref(),
        )
        .await
        {
            tracing::error!(
                error = %e,
                wallet = %signal.wallet,
                market = %signal.market_id,
                "Copy trade execution failed"
            );
        }
    }

    tracing::warn!("Copy engine channel closed — shutting down");
}

async fn process_signal(
    signal: &CopySignal,
    pool: &PgPool,
    executor: &OrderExecutor,
    config: &CopyEngineConfig,
    notifier: Option<&Notifier>,
    balance_checker: Option<&BalanceChecker>,
) -> anyhow::Result<()> {
    // 1. Calculate position size
    let signal_strength = signal.whale_win_rate; // Use win rate as signal strength
    let size = position_sizer::calculate_size(
        config.strategy,
        config.bankroll,
        signal.whale_notional,
        signal.whale_win_rate,
        signal.whale_kelly,
        config.base_amount,
        signal_strength,
    );

    if size <= Decimal::ZERO {
        tracing::debug!(wallet = %signal.wallet, "Calculated size is zero, skipping");
        return Ok(());
    }

    tracing::info!(
        strategy = %config.strategy,
        size = %size,
        "Position sized"
    );

    // 1b. Balance pre-check (only when not dry-run and checker available)
    if !config.dry_run {
        if let Some(checker) = balance_checker {
            let side_str = signal.side.to_string();
            match side_str.as_str() {
                "BUY" => {
                    let required = size * signal.price;
                    match checker.get_usdc_balance().await {
                        Ok(usdc) if usdc < required => {
                            tracing::warn!(
                                required = %required,
                                available = %usdc,
                                "Insufficient USDC balance — skipping order"
                            );
                            return Ok(());
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Failed to check USDC balance, proceeding anyway");
                        }
                        _ => {}
                    }
                }
                "SELL" => {
                    match checker.get_token_balance(&signal.asset_id).await {
                        Ok(token_bal) if token_bal < size => {
                            tracing::warn!(
                                required = %size,
                                available = %token_bal,
                                token_id = %signal.asset_id,
                                "Insufficient token balance — skipping order"
                            );
                            return Ok(());
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Failed to check token balance, proceeding anyway");
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    // 2. Build portfolio snapshot for risk check
    let open_positions = position_repo::count_open_positions(pool).await.unwrap_or(0);
    let daily_pnl = position_repo::get_daily_realized_pnl(pool).await.unwrap_or(Decimal::ZERO);

    let portfolio = PortfolioSnapshot {
        bankroll: config.bankroll,
        open_positions,
        daily_pnl,
    };

    let pending_order = PendingOrder {
        size,
        price: signal.price,
    };

    // 3. Risk check
    if let Err(violation) = risk_manager::check_risk(
        &pending_order,
        &portfolio,
        &config.risk_limits,
    ) {
        tracing::warn!(
            violation = %violation,
            wallet = %signal.wallet,
            "Risk check failed — order rejected"
        );
        return Ok(());
    }

    tracing::info!("Risk check passed");

    // 4. Record order in DB
    let side_str = signal.side.to_string();
    let order = order_repo::insert_order(
        pool,
        signal.whale_trade_id,
        &signal.market_id,
        &signal.asset_id,
        &side_str,
        size,
        signal.price,
        &config.strategy.to_string(),
    )
    .await?;

    tracing::info!(order_id = %order.id, "Order recorded");

    // 5. Execute
    match executor.execute(&signal.asset_id, &side_str, size, signal.price).await {
        Ok(result) => {
            tracing::info!(
                order_id = %order.id,
                fill_price = %result.fill_price,
                slippage = %result.slippage,
                clob_order_id = ?result.order_id,
                "Order executed successfully"
            );

            counter!("orders_filled").increment(1);

            // Update order as filled
            order_repo::fill_order(pool, order.id, result.fill_price, result.slippage).await?;

            // Update/create position
            let outcome = match signal.side {
                crate::models::Side::Buy => "Yes",
                crate::models::Side::Sell => "No",
            };

            let position = position_repo::upsert_position(
                pool,
                &signal.market_id,
                &signal.asset_id,
                outcome,
                size,
                result.fill_price,
            )
            .await?;

            // Set SL/TP on the position
            if let Err(e) = position_repo::set_position_sl_tp(
                pool,
                position.id,
                config.default_stop_loss_pct,
                config.default_take_profit_pct,
            )
            .await
            {
                tracing::warn!(error = %e, "Failed to set SL/TP on position");
            }

            tracing::info!(order_id = %order.id, "Position updated");

            // Notify order result
            if let Some(n) = notifier {
                let market_question = market_repo::get_market_question(pool, &signal.market_id)
                    .await
                    .ok()
                    .flatten();
                let msg = crate::services::notifier::format_order_result(&order, true, None, market_question.as_deref());
                n.send(&msg).await;
            }
        }
        Err(e) => {
            let err_msg = e.to_string();
            tracing::error!(
                order_id = %order.id,
                error = %err_msg,
                "Order execution failed"
            );

            counter!("orders_failed").increment(1);
            order_repo::fail_order(pool, order.id, &err_msg).await?;

            // Notify order failure
            if let Some(n) = notifier {
                let market_question = market_repo::get_market_question(pool, &signal.market_id)
                    .await
                    .ok()
                    .flatten();
                let msg = crate::services::notifier::format_order_result(&order, false, Some(&err_msg), market_question.as_deref());
                n.send(&msg).await;
            }
        }
    }

    Ok(())
}
