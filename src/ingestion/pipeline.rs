use metrics::{counter, histogram};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::time::Instant;
use tokio::sync::mpsc;

use crate::db::{basket_repo, market_repo, trade_repo, whale_repo};
use crate::intelligence::basket::check_basket_consensus;
use crate::intelligence::classifier::Classification;
use crate::intelligence::{classify_wallet, score_wallet};
use crate::models::{CopySignal, Side, TradeResult, WhaleTradeEvent};
use crate::services::notifier::Notifier;

/// Minimum notional value (in USDC) to consider a trade whale-grade.
const WHALE_NOTIONAL_THRESHOLD: i64 = 10_000;

/// Process a single WhaleTradeEvent through the intelligence pipeline:
/// 1. Filter by notional threshold
/// 2. Upsert whale record
/// 3. Persist trade to DB
/// 4. Re-score and re-classify the wallet
/// 5. Emit CopySignal if wallet qualifies
pub async fn process_trade_event(
    event: &WhaleTradeEvent,
    pool: &PgPool,
    signal_tx: Option<&mpsc::Sender<CopySignal>>,
    notifier: Option<&Notifier>,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let threshold = Decimal::from(WHALE_NOTIONAL_THRESHOLD);

    // Step 1: Filter by notional value
    if event.notional < threshold {
        tracing::debug!(
            wallet = %event.wallet,
            notional = %event.notional,
            "Trade below whale threshold, skipping"
        );
        return Ok(());
    }

    tracing::info!(
        wallet = %event.wallet,
        market = %event.market_id,
        side = %event.side,
        notional = %event.notional,
        "Whale-grade trade detected"
    );

    counter!("trade_events_total").increment(1);

    // Notify whale alert
    if let Some(n) = notifier {
        let msg = crate::services::notifier::format_whale_alert(event);
        n.send(&msg).await;
    }

    // Step 2: Upsert whale
    let whale = whale_repo::upsert_whale(pool, &event.wallet).await?;

    // Step 3: Persist trade
    let trade = trade_repo::insert_trade(
        pool,
        whale.id,
        &event.market_id,
        &event.asset_id,
        &event.side.to_string(),
        event.size,
        event.price,
        event.notional,
        event.timestamp,
    )
    .await?;

    // Ensure market_outcome record exists for this market
    let _ = market_repo::upsert_market_outcome(pool, &event.market_id, Some(&event.asset_id)).await;

    // Update last_trade_at
    whale_repo::touch_whale_last_trade(pool, whale.id, event.timestamp).await?;

    // Step 4: Fetch trade history and re-score
    let all_trades = trade_repo::get_trades_by_whale(pool, whale.id).await?;

    // Classify wallet
    let classification = classify_wallet(&all_trades);
    whale_repo::update_whale_classification(pool, whale.id, classification.as_str()).await?;

    // Score wallet — use real market outcomes when available
    let trade_results: Vec<TradeResult> = {
        let mut results = Vec::with_capacity(all_trades.len());
        for t in &all_trades {
            let outcome = market_repo::get_market_outcome(pool, &t.market_id).await.ok().flatten();
            let profit = match outcome.as_ref().map(|o| o.outcome.as_str()) {
                Some("resolved_yes") => {
                    if t.side == "BUY" {
                        // BUY YES wins: profit = notional * (1 - price) / price
                        t.notional * (Decimal::ONE - t.price) / t.price
                    } else {
                        // SELL YES loses: loss = -notional
                        -t.notional
                    }
                }
                Some("resolved_no") => {
                    if t.side == "BUY" {
                        // BUY YES loses: loss = -notional
                        -t.notional
                    } else {
                        // SELL YES wins: profit = notional * price / (1 - price)
                        t.notional * t.price / (Decimal::ONE - t.price)
                    }
                }
                _ => {
                    // Unresolved — don't count toward win/loss
                    Decimal::ZERO
                }
            };
            results.push(TradeResult {
                profit,
                traded_at: t.traded_at,
            });
        }
        results
    };

    // Only include resolved trades for scoring (filter out zero-profit unresolved)
    let resolved_results: Vec<TradeResult> = trade_results
        .into_iter()
        .filter(|r| r.profit != Decimal::ZERO)
        .collect();

    if resolved_results.is_empty() {
        // No resolved trades yet — still persist classification and continue
        let elapsed = start.elapsed().as_secs_f64();
        histogram!("pipeline_latency_seconds").record(elapsed);
        return Ok(());
    }

    let score = score_wallet(&resolved_results);

    whale_repo::update_whale_scores(
        pool,
        whale.id,
        score.sharpe_ratio,
        score.win_rate,
        score.kelly_fraction,
        score.expected_value,
        score.total_trades,
        score.total_pnl,
    )
    .await?;

    tracing::info!(
        wallet = %event.wallet,
        classification = %classification,
        sharpe = %score.sharpe_ratio,
        win_rate = %score.win_rate,
        kelly = %score.kelly_fraction,
        ev = %score.expected_value,
        trades = score.total_trades,
        decaying = score.is_decaying,
        "Wallet scored"
    );

    // Auto-deactivate if decaying
    if score.is_decaying {
        tracing::warn!(
            wallet = %event.wallet,
            "Wallet performance decaying — deactivating"
        );
        whale_repo::deactivate_whale(pool, whale.id).await?;
        let elapsed = start.elapsed().as_secs_f64();
        histogram!("pipeline_latency_seconds").record(elapsed);
        return Ok(());
    }

    // Step 5: Emit CopySignal if wallet is Informed and active
    if classification == Classification::Informed && whale.is_active.unwrap_or(true) {
        if let Some(tx) = signal_tx {
            let signal = CopySignal {
                whale_trade_id: trade.id,
                wallet: event.wallet.clone(),
                market_id: event.market_id.clone(),
                asset_id: event.asset_id.clone(),
                side: event.side,
                price: event.price,
                whale_win_rate: score.win_rate,
                whale_kelly: score.kelly_fraction,
                whale_notional: event.notional,
            };

            if let Err(e) = tx.send(signal).await {
                tracing::error!(error = %e, "Failed to send CopySignal to execution layer");
            } else {
                counter!("copy_signals_emitted").increment(1);
                tracing::info!(
                    wallet = %event.wallet,
                    market = %event.market_id,
                    "CopySignal emitted to execution layer"
                );
            }
        }
    }

    // Step 6: Basket consensus check
    // For each basket this whale belongs to, evaluate consensus
    if let Ok(baskets) = basket_repo::get_baskets_for_whale(pool, whale.id).await {
        for basket in &baskets {
            match check_basket_consensus(pool, basket, &event.market_id, event.price).await {
                Ok(check) => {
                    if check.reached {
                        tracing::info!(
                            basket = %basket.name,
                            market = %event.market_id,
                            direction = %check.direction,
                            pct = %check.consensus_pct,
                            participants = check.participating,
                            total = check.total,
                            "Basket consensus reached"
                        );

                        counter!("consensus_signals_total").increment(1);

                        // Notify consensus
                        if let Some(n) = notifier {
                            let msg = crate::services::notifier::format_consensus_alert(
                                &basket.name,
                                &check.direction,
                                check.consensus_pct,
                            );
                            n.send(&msg).await;
                        }

                        // Record consensus signal
                        if let Err(e) = basket_repo::record_consensus_signal(
                            pool,
                            basket.id,
                            &event.market_id,
                            &check.direction,
                            check.consensus_pct,
                            check.participating,
                            check.total,
                        )
                        .await
                        {
                            tracing::error!(error = %e, "Failed to record consensus signal");
                        }

                        // Emit enhanced CopySignal from basket
                        if let Some(tx) = signal_tx {
                            let side = Side::from_api_str(&check.direction)
                                .unwrap_or(Side::Buy);

                            let basket_signal = CopySignal {
                                whale_trade_id: trade.id,
                                wallet: format!("basket:{}", basket.name),
                                market_id: event.market_id.clone(),
                                asset_id: event.asset_id.clone(),
                                side,
                                price: event.price,
                                whale_win_rate: score.win_rate,
                                whale_kelly: score.kelly_fraction,
                                whale_notional: event.notional,
                            };

                            if let Err(e) = tx.send(basket_signal).await {
                                tracing::error!(error = %e, "Failed to send basket CopySignal");
                            }
                        }
                    } else {
                        tracing::debug!(
                            basket = %basket.name,
                            market = %event.market_id,
                            reason = %check.reason,
                            "Basket consensus not reached"
                        );
                    }
                }
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        basket = %basket.name,
                        "Failed to check basket consensus"
                    );
                }
            }
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    histogram!("pipeline_latency_seconds").record(elapsed);

    Ok(())
}
