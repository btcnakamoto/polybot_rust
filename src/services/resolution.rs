use std::sync::Arc;

use rust_decimal::Decimal;
use sqlx::PgPool;
use tokio::time::{interval, sleep, Duration};

use crate::db::{market_repo, position_repo};
use crate::polymarket::DataClient;
use crate::services::notifier::Notifier;

/// Max markets to check per cycle (avoid rate limits).
const BATCH_SIZE: usize = 50;

/// Delay between API calls to respect rate limits.
const API_DELAY: Duration = Duration::from_millis(200);

/// Periodically poll unresolved markets and settle positions when outcomes are known.
pub async fn run_resolution_poller(
    pool: PgPool,
    data_client: DataClient,
    interval_secs: u64,
    notifier: Option<Arc<Notifier>>,
) {
    let mut ticker = interval(Duration::from_secs(interval_secs));

    loop {
        ticker.tick().await;

        let unresolved = match market_repo::get_unresolved_markets(&pool).await {
            Ok(m) => m,
            Err(e) => {
                tracing::error!(error = %e, "Failed to fetch unresolved markets");
                continue;
            }
        };

        if unresolved.is_empty() {
            tracing::info!("Resolution poller: no unresolved markets");
            continue;
        }

        let batch = &unresolved[..unresolved.len().min(BATCH_SIZE)];
        tracing::info!(
            total = unresolved.len(),
            checking = batch.len(),
            "Resolution poller: checking markets"
        );

        let mut resolved_count = 0u32;
        let mut failed_count = 0u32;
        let mut still_open = 0u32;

        for market_outcome in batch {
            match data_client.get_market_for_resolution(&market_outcome.market_id).await {
                Ok(api_market) => {
                    // Check if market is closed
                    if api_market.closed != Some(true) {
                        still_open += 1;
                        continue;
                    }

                    // Find winning token
                    let mut resolved_outcome: Option<&str> = None;
                    for token in &api_market.tokens {
                        if token.winner == Some(true) {
                            let outcome_upper = token.outcome.to_uppercase();
                            if outcome_upper == "YES" {
                                resolved_outcome = Some("resolved_yes");
                            } else if outcome_upper == "NO" {
                                resolved_outcome = Some("resolved_no");
                            }
                            break;
                        }
                    }

                    let Some(outcome_str) = resolved_outcome else {
                        // Market closed but no winner declared yet
                        still_open += 1;
                        continue;
                    };

                    tracing::info!(
                        market_id = %market_outcome.market_id,
                        outcome = outcome_str,
                        question = %api_market.question,
                        "Market resolved"
                    );

                    // Update market_outcomes table
                    if let Err(e) = market_repo::resolve_market(&pool, &market_outcome.market_id, outcome_str).await {
                        tracing::error!(error = %e, market_id = %market_outcome.market_id, "Failed to resolve market");
                        continue;
                    }

                    resolved_count += 1;

                    // Settle positions for this market
                    let positions = match position_repo::get_positions_for_market(&pool, &market_outcome.market_id).await {
                        Ok(p) => p,
                        Err(e) => {
                            tracing::error!(error = %e, "Failed to get positions for market");
                            continue;
                        }
                    };

                    for pos in &positions {
                        let pnl = if outcome_str == "resolved_yes" {
                            if pos.outcome == "Yes" {
                                pos.size * (Decimal::ONE - pos.avg_entry_price)
                            } else {
                                -(pos.size * pos.avg_entry_price)
                            }
                        } else {
                            if pos.outcome == "No" {
                                pos.size * (Decimal::ONE - pos.avg_entry_price)
                            } else {
                                -(pos.size * pos.avg_entry_price)
                            }
                        };

                        if let Err(e) = position_repo::close_position(&pool, pos.id, pnl).await {
                            tracing::error!(
                                error = %e,
                                position_id = %pos.id,
                                "Failed to close position"
                            );
                        } else {
                            tracing::info!(
                                position_id = %pos.id,
                                market_id = %market_outcome.market_id,
                                pnl = %pnl,
                                "Position settled"
                            );
                        }
                    }

                    // Notify settlement
                    if let Some(ref n) = notifier {
                        let total_pnl: Decimal = positions.iter().map(|p| {
                            if outcome_str == "resolved_yes" {
                                if p.outcome == "Yes" {
                                    p.size * (Decimal::ONE - p.avg_entry_price)
                                } else {
                                    -(p.size * p.avg_entry_price)
                                }
                            } else if p.outcome == "No" {
                                p.size * (Decimal::ONE - p.avg_entry_price)
                            } else {
                                -(p.size * p.avg_entry_price)
                            }
                        }).sum();

                        if !positions.is_empty() {
                            let market_question = market_repo::get_market_question(&pool, &market_outcome.market_id)
                                .await
                                .ok()
                                .flatten();
                            let msg = crate::services::notifier::format_market_settled(
                                market_question.as_deref(),
                                &market_outcome.market_id,
                                outcome_str,
                                positions.len(),
                                total_pnl,
                            );
                            n.send(&msg).await;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        market_id = %market_outcome.market_id,
                        "Resolution: market lookup failed"
                    );
                    failed_count += 1;
                }
            }

            // Rate limit: small delay between API calls
            sleep(API_DELAY).await;
        }

        tracing::info!(
            resolved = resolved_count,
            still_open = still_open,
            failed = failed_count,
            remaining = unresolved.len().saturating_sub(BATCH_SIZE),
            "Resolution poller cycle complete"
        );
    }
}
