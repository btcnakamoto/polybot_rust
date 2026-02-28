use chrono::Utc;
use metrics::{counter, histogram};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::db::{basket_repo, market_repo, trade_repo, whale_repo};
use crate::intelligence::basket::{check_admission, check_basket_consensus, AdmissionResult};
use crate::intelligence::classifier::Classification;
use crate::intelligence::{classify_wallet, score_wallet};
use crate::intelligence::scorer::WalletScore;
use crate::models::{CopySignal, Side, TradeResult, WhaleTradeEvent};
use crate::services::notifier::Notifier;

/// Minimum notional value (in USDC) to consider a trade from an UNKNOWN wallet.
const WHALE_NOTIONAL_THRESHOLD: i64 = 10_000;

/// Pipeline configuration for signal quality gates.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub tracked_whale_min_notional: Decimal,
    pub min_signal_win_rate: Decimal,
    pub min_resolved_for_signal: i32,
    pub min_total_trades_for_signal: i32,
    pub min_signal_notional: Decimal,
    pub max_signal_notional: Decimal,
    pub min_signal_ev: Decimal,
    pub assumed_slippage_pct: Decimal,
    pub signal_dedup_window_secs: u64,
}

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
    config: &PipelineConfig,
    dedup: &tokio::sync::Mutex<HashMap<String, Instant>>,
) -> anyhow::Result<()> {
    let start = Instant::now();

    // Step 1: Filter by notional value
    // Use lower threshold for already-tracked whales (from seeder/poller)
    let is_tracked = whale_repo::get_whale_by_address(pool, &event.wallet)
        .await
        .ok()
        .flatten()
        .map(|w| w.is_active.unwrap_or(false))
        .unwrap_or(false);

    let threshold = if is_tracked {
        config.tracked_whale_min_notional
    } else {
        Decimal::from(WHALE_NOTIONAL_THRESHOLD)
    };

    if event.notional < threshold {
        tracing::debug!(
            wallet = %event.wallet,
            notional = %event.notional,
            tracked = is_tracked,
            "Trade below threshold, skipping"
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
    let resolved_count = resolved_results.len() as i32;

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

    // Step 6: Emit CopySignal if wallet passes classification, validated scores,
    // total trades, notional range, and win rate gates.
    let is_valid_classification = classification != Classification::Bot
        && classification != Classification::MarketMaker;

    let has_validated_scores = resolved_count >= config.min_resolved_for_signal;

    // Effective total trades: max of observed trades and seeded/leaderboard total
    let effective_total_trades = (all_trades.len() as i32).max(score.total_trades);

    let has_enough_total_trades = effective_total_trades >= config.min_total_trades_for_signal;

    // EV_copy = EV * (1 - assumed_slippage) — slippage-adjusted expected value per trade
    let ev_copy = score.expected_value * (Decimal::ONE - config.assumed_slippage_pct);
    let has_sufficient_ev = ev_copy >= config.min_signal_ev;

    if !is_valid_classification {
        tracing::info!(
            wallet = %event.wallet,
            classification = %classification,
            "Signal blocked: classified as {}",
            classification.as_str()
        );
    } else if !has_validated_scores {
        tracing::info!(
            wallet = %event.wallet,
            resolved = resolved_count,
            required = config.min_resolved_for_signal,
            "Signal blocked: only {} resolved trades (need {})",
            resolved_count,
            config.min_resolved_for_signal
        );
    } else if !has_enough_total_trades {
        tracing::info!(
            wallet = %event.wallet,
            total_trades = effective_total_trades,
            required = config.min_total_trades_for_signal,
            "Signal blocked: only {} total trades (need {})",
            effective_total_trades,
            config.min_total_trades_for_signal
        );
    } else if event.notional < config.min_signal_notional {
        tracing::info!(
            wallet = %event.wallet,
            notional = %event.notional,
            min = %config.min_signal_notional,
            "Signal blocked: notional ${} below ${} minimum",
            event.notional,
            config.min_signal_notional
        );
    } else if event.notional > config.max_signal_notional {
        tracing::info!(
            wallet = %event.wallet,
            notional = %event.notional,
            max = %config.max_signal_notional,
            "Signal blocked: notional ${} above ${} maximum",
            event.notional,
            config.max_signal_notional
        );
    } else if !has_sufficient_ev {
        tracing::info!(
            wallet = %event.wallet,
            ev = %score.expected_value,
            ev_copy = %ev_copy,
            min = %config.min_signal_ev,
            slippage_pct = %config.assumed_slippage_pct,
            "Signal blocked: EV_copy ${} below ${} minimum (EV=${}, slippage={}%)",
            ev_copy,
            config.min_signal_ev,
            score.expected_value,
            config.assumed_slippage_pct * Decimal::ONE_HUNDRED
        );
    } else if score.win_rate >= config.min_signal_win_rate && whale.is_active.unwrap_or(true) {
        // Dedup check: skip if same (wallet, asset_id, side) emitted within window
        let dedup_key = format!("{}:{}:{}", event.wallet, event.asset_id, event.side);
        let is_dup = {
            let mut dedup_map = dedup.lock().await;
            dedup_map.retain(|_, t| t.elapsed() < Duration::from_secs(config.signal_dedup_window_secs));
            use std::collections::hash_map::Entry;
            match dedup_map.entry(dedup_key.clone()) {
                Entry::Occupied(_) => true,
                Entry::Vacant(e) => {
                    e.insert(Instant::now());
                    false
                }
            }
        };

        if is_dup {
            tracing::debug!(key = %dedup_key, "Signal deduped — skipping");
        } else if let Some(tx) = signal_tx {
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

                // Notify copy signal via Telegram
                if let Some(n) = notifier {
                    let msg = crate::services::notifier::format_copy_signal(
                        event,
                        score.win_rate,
                        score.kelly_fraction,
                        ev_copy,
                    );
                    n.send(&msg).await;
                }
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
