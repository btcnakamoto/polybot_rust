use chrono::Utc;
use metrics::{counter, histogram};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::time::Instant;
use tokio::sync::mpsc;

use crate::db::{basket_repo, market_repo, trade_repo, whale_repo};
use crate::intelligence::basket::{check_admission, check_basket_consensus, AdmissionResult};
use crate::intelligence::classifier::Classification;
use crate::intelligence::{classify_wallet, score_wallet};
use crate::intelligence::scorer::WalletScore;
use crate::models::{CopySignal, Side, TradeResult, WhaleTradeEvent};
use crate::services::notifier::Notifier;

/// Minimum notional value (in USDC) to consider a trade whale-grade.
const WHALE_NOTIONAL_THRESHOLD: i64 = 10_000;

/// Process a single WhaleTradeEvent through the intelligence pipeline:
/// 1. Filter by notional threshold
/// 2. Upsert whale record
/// 3. Persist trade to DB
/// 4. Re-score and re-classify the wallet
/// 5. Basket admission check
/// 6. Emit CopySignal if wallet qualifies
/// 7. Basket consensus check
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
                        t.notional * (Decimal::ONE - t.price) / t.price
                    } else {
                        -t.notional
                    }
                }
                Some("resolved_no") => {
                    if t.side == "BUY" {
                        -t.notional
                    } else {
                        t.notional * t.price / (Decimal::ONE - t.price)
                    }
                }
                _ => Decimal::ZERO,
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

    // Build score: prefer resolved trade data, fall back to existing DB scores (from seeder)
    let score = if !resolved_results.is_empty() {
        // We have resolved trades — compute fresh scores
        let s = score_wallet(&resolved_results);

        whale_repo::update_whale_scores(
            pool,
            whale.id,
            s.sharpe_ratio,
            s.win_rate,
            s.kelly_fraction,
            s.expected_value,
            s.total_trades,
            s.total_pnl,
        )
        .await?;

        Some(s)
    } else if whale.win_rate.is_some() && whale.win_rate != Some(Decimal::ZERO) {
        // No resolved trades yet, but whale has existing scores from seeder/leaderboard.
        // Use those scores so the pipeline can still emit signals.
        let win_rate = whale.win_rate.unwrap_or(Decimal::ZERO);
        let kelly = whale.kelly_fraction.unwrap_or(Decimal::ZERO);
        let total_trades = whale.total_trades.unwrap_or(0);
        let total_pnl = whale.total_pnl.unwrap_or(Decimal::ZERO);

        tracing::debug!(
            wallet = %event.wallet,
            win_rate = %win_rate,
            kelly = %kelly,
            "Using existing DB scores (no resolved market outcomes yet)"
        );

        Some(WalletScore {
            sharpe_ratio: whale.sharpe_ratio.unwrap_or(Decimal::ZERO),
            win_rate,
            kelly_fraction: kelly,
            expected_value: whale.expected_value.unwrap_or(Decimal::ZERO),
            total_trades,
            total_pnl,
            is_decaying: false,
        })
    } else {
        // No resolved trades AND no existing scores — nothing to work with
        tracing::debug!(
            wallet = %event.wallet,
            "No resolved trades and no existing scores — skipping signal emission"
        );
        None
    };

    let score = match score {
        Some(s) => s,
        None => {
            let elapsed = start.elapsed().as_secs_f64();
            histogram!("pipeline_latency_seconds").record(elapsed);
            return Ok(());
        }
    };

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

    // Step 5: Basket admission check
    // README: "find 5-10 wallets with >60% win rate and >4 months of history"
    // "filter out bots (>100 trades/month = probably automated)"
    //
    // Use earliest trade timestamp for months_active (not whale.created_at which is
    // just when we added the record to our DB).
    let months_active = {
        let earliest = all_trades
            .iter()
            .map(|t| t.traded_at)
            .min()
            .unwrap_or_else(Utc::now);
        let diff = Utc::now().signed_duration_since(earliest);
        (diff.num_days() / 30).max(1)
    };
    let avg_monthly_trades = if months_active > 0 {
        Decimal::from(score.total_trades) / Decimal::from(months_active)
    } else {
        Decimal::from(score.total_trades)
    };

    let admission = check_admission(
        score.win_rate,
        Some(classification.as_str()),
        months_active,
        score.total_trades,
        avg_monthly_trades,
    );

    let admitted = matches!(admission, AdmissionResult::Accepted);
    if !admitted {
        if let AdmissionResult::Rejected(ref reason) = admission {
            tracing::info!(
                wallet = %event.wallet,
                reason = %reason,
                "Wallet failed basket admission — will not participate in consensus"
            );
        }
    }

    // Step 6: Emit CopySignal if wallet is Informed, active, AND admitted
    if classification == Classification::Informed && whale.is_active.unwrap_or(true) && admitted {
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

    // Step 7: Basket consensus check (only if wallet passed admission)
    if !admitted {
        tracing::debug!(
            wallet = %event.wallet,
            "Skipping basket consensus — wallet not admitted"
        );
        let elapsed = start.elapsed().as_secs_f64();
        histogram!("pipeline_latency_seconds").record(elapsed);
        return Ok(());
    }

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
