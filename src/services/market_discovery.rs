use rust_decimal::Decimal;
use std::str::FromStr;
use sqlx::PgPool;
use tokio::sync::watch;
use tokio::time::{interval, Duration};

use crate::polymarket::gamma_client::GammaClient;

/// Run the market discovery loop. Periodically fetches active markets from the
/// Gamma API, filters by volume/liquidity thresholds, and broadcasts the
/// resulting token IDs to the WS listener via a `watch` channel.
pub async fn run_market_discovery(
    gamma_client: GammaClient,
    token_tx: watch::Sender<Vec<String>>,
    pool: PgPool,
    interval_secs: u64,
    min_volume: Decimal,
    min_liquidity: Decimal,
) {
    let mut ticker = interval(Duration::from_secs(interval_secs));

    loop {
        ticker.tick().await;

        tracing::info!("Market discovery: scanning for active markets");

        let mut all_token_ids: Vec<String> = Vec::new();
        let mut markets_found: usize = 0;
        let mut offset: u32 = 0;
        let limit: u32 = 100;

        // Paginate through all active markets
        loop {
            match gamma_client.get_active_markets(limit, offset).await {
                Ok(markets) => {
                    let batch_len = markets.len();

                    for market in &markets {
                        let volume = market
                            .volume
                            .as_deref()
                            .and_then(|v| Decimal::from_str(v).ok())
                            .unwrap_or(Decimal::ZERO);

                        let liquidity = market
                            .liquidity
                            .as_deref()
                            .and_then(|v| Decimal::from_str(v).ok())
                            .unwrap_or(Decimal::ZERO);

                        if volume >= min_volume && liquidity >= min_liquidity {
                            markets_found += 1;
                            for token_id in market.parse_token_ids() {
                                if !token_id.is_empty() {
                                    all_token_ids.push(token_id);
                                }
                            }

                            // Persist to active_markets table for dashboard
                            if let Err(e) = upsert_active_market(
                                &pool,
                                &market.condition_id,
                                &market.question,
                                volume,
                                liquidity,
                                market.end_date_iso.as_deref(),
                                market.clob_token_ids.as_deref(),
                                market.event_slug(),
                                market.outcomes_str(),
                            )
                            .await
                            {
                                tracing::warn!(
                                    error = %e,
                                    condition_id = %market.condition_id,
                                    "Failed to persist active market"
                                );
                            }
                        }
                    }

                    if batch_len < limit as usize {
                        break; // No more pages
                    }
                    offset += limit;
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to fetch markets from Gamma API");
                    break;
                }
            }
        }

        // Deduplicate
        all_token_ids.sort();
        all_token_ids.dedup();

        let token_count = all_token_ids.len();
        tracing::info!(
            markets = markets_found,
            tokens = token_count,
            "Discovered {} active markets with {} tokens",
            markets_found,
            token_count,
        );

        // Broadcast updated token list to WS listener
        if !all_token_ids.is_empty() {
            if let Err(e) = token_tx.send(all_token_ids) {
                tracing::error!(error = %e, "Failed to broadcast token IDs");
            }
        }
    }
}

/// Upsert a market into the active_markets table.
async fn upsert_active_market(
    pool: &PgPool,
    condition_id: &str,
    question: &str,
    volume: Decimal,
    liquidity: Decimal,
    end_date_iso: Option<&str>,
    clob_token_ids: Option<&str>,
    slug: Option<&str>,
    outcomes: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO active_markets (condition_id, question, volume, liquidity, end_date_iso, clob_token_ids, slug, outcomes, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
        ON CONFLICT (condition_id) DO UPDATE
        SET question = EXCLUDED.question,
            volume = EXCLUDED.volume,
            liquidity = EXCLUDED.liquidity,
            end_date_iso = EXCLUDED.end_date_iso,
            clob_token_ids = EXCLUDED.clob_token_ids,
            slug = EXCLUDED.slug,
            outcomes = EXCLUDED.outcomes,
            updated_at = NOW()
        "#,
    )
    .bind(condition_id)
    .bind(question)
    .bind(volume)
    .bind(liquidity)
    .bind(end_date_iso)
    .bind(clob_token_ids)
    .bind(slug)
    .bind(outcomes)
    .execute(pool)
    .await?;

    Ok(())
}
