use std::sync::Arc;

use rust_decimal::Decimal;
use sqlx::PgPool;
use tokio::time::{interval, Duration};

use crate::db::{market_repo, position_repo};
use crate::polymarket::DataClient;
use crate::services::notifier::Notifier;

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

        tracing::debug!("Resolution poller: checking unresolved markets");

        let unresolved = match market_repo::get_unresolved_markets(&pool).await {
            Ok(m) => m,
            Err(e) => {
                tracing::error!(error = %e, "Failed to fetch unresolved markets");
                continue;
            }
        };

        if unresolved.is_empty() {
            tracing::debug!("No unresolved markets to check");
            continue;
        }

        for market_outcome in &unresolved {
            match data_client.get_market(&market_outcome.market_id).await {
                Ok(api_market) => {
                    // Check if market is closed
                    if api_market.closed != Some(true) {
                        continue;
                    }

                    // Find winning token
                    let mut resolved_outcome: Option<&str> = None;
                    for token in &api_market.tokens {
                        if token.winner == Some(true) {
                            // Determine if YES or NO won based on outcome field
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
                        continue;
                    };

                    tracing::info!(
                        market_id = %market_outcome.market_id,
                        outcome = outcome_str,
                        "Market resolved"
                    );

                    // Update market_outcomes table
                    if let Err(e) = market_repo::resolve_market(&pool, &market_outcome.market_id, outcome_str).await {
                        tracing::error!(error = %e, market_id = %market_outcome.market_id, "Failed to resolve market");
                        continue;
                    }

                    // Settle positions for this market
                    let positions = match position_repo::get_positions_for_market(&pool, &market_outcome.market_id).await {
                        Ok(p) => p,
                        Err(e) => {
                            tracing::error!(error = %e, "Failed to get positions for market");
                            continue;
                        }
                    };

                    for pos in &positions {
                        // Calculate realized PnL based on outcome
                        let pnl = if outcome_str == "resolved_yes" {
                            if pos.outcome == "Yes" {
                                // Bought YES and YES won: profit = size * (1 - entry_price)
                                pos.size * (Decimal::ONE - pos.avg_entry_price)
                            } else {
                                // Bought NO and YES won: loss = -size * entry_price
                                -(pos.size * pos.avg_entry_price)
                            }
                        } else {
                            // resolved_no
                            if pos.outcome == "No" {
                                // Bought NO and NO won: profit = size * (1 - entry_price)
                                pos.size * (Decimal::ONE - pos.avg_entry_price)
                            } else {
                                // Bought YES and NO won: loss = -size * entry_price
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
                            let msg = format!(
                                "*Market Settled*\nMarket: `{}`\nOutcome: {}\nPositions closed: {}\nTotal PnL: {} USDC",
                                market_outcome.market_id,
                                outcome_str.replace('_', " "),
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
                        "Failed to fetch market from API — will retry"
                    );
                }
            }
        }
    }
}
