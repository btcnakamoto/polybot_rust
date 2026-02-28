use std::collections::HashMap;
use std::time::Duration;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::db::whale_repo;
use crate::models::{Side, WhaleTradeEvent};
use crate::polymarket::DataClient;

/// Poll each tracked whale's recent trades via the Data API.
///
/// This is the primary mechanism for detecting whale trades in real-time,
/// since the Polymarket WebSocket doesn't include wallet addresses.
///
/// Flow:
/// 1. Every `interval_secs`, fetch active whales from DB
/// 2. For each whale, query their recent trades from the Data API
/// 3. Compare with last-seen trade timestamp to find new trades
/// 4. Send new trades to the pipeline via the `trade_tx` channel
pub async fn run_whale_trade_poller(
    data_client: DataClient,
    pool: PgPool,
    trade_tx: mpsc::Sender<WhaleTradeEvent>,
    interval_secs: u64,
) {
    tracing::info!(
        interval_secs = interval_secs,
        "Whale trade poller started"
    );

    // Track last seen trade timestamp per whale address
    let mut last_seen: HashMap<String, DateTime<Utc>> = HashMap::new();

    // Initialize last_seen to now so we only capture NEW trades
    if let Ok(whales) = whale_repo::get_active_whales(&pool).await {
        for whale in &whales {
            last_seen.insert(whale.address.clone(), Utc::now());
        }
        tracing::info!(
            whale_count = whales.len(),
            "Initialized whale trade poller with {} whales",
            whales.len()
        );
    }

    loop {
        sleep(Duration::from_secs(interval_secs)).await;

        let whales = match whale_repo::get_active_whales(&pool).await {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(error = %e, "Whale poller: failed to fetch active whales");
                continue;
            }
        };

        let mut total_new_trades = 0u32;

        for whale in &whales {
            let trades = match data_client.get_user_trades(&whale.address, 10).await {
                Ok(t) => t,
                Err(e) => {
                    tracing::debug!(
                        error = %e,
                        address = %whale.address,
                        "Whale poller: failed to fetch trades"
                    );
                    continue;
                }
            };

            let cutoff = last_seen
                .get(&whale.address)
                .copied()
                .unwrap_or_else(Utc::now);

            let mut latest_ts = cutoff;

            for trade in &trades {
                let traded_at = parse_trade_timestamp(trade.timestamp.as_ref())
                    .unwrap_or_else(Utc::now);

                // Skip trades we've already seen
                if traded_at <= cutoff {
                    continue;
                }

                if traded_at > latest_ts {
                    latest_ts = traded_at;
                }

                let side_str = trade.side.as_deref().unwrap_or("BUY");
                let side = match Side::from_api_str(side_str) {
                    Some(s) => s,
                    None => continue,
                };

                let token_id = trade.token_id.as_deref().unwrap_or("unknown");
                let market_id = trade.market.as_deref().unwrap_or("unknown");
                let size = trade.size.unwrap_or(Decimal::ZERO);
                let price = trade.price.unwrap_or(Decimal::ZERO);
                let notional = size * price;

                let event = WhaleTradeEvent {
                    wallet: whale.address.clone(),
                    market_id: market_id.to_string(),
                    asset_id: token_id.to_string(),
                    side,
                    size,
                    price,
                    notional,
                    timestamp: traded_at,
                };

                tracing::info!(
                    wallet = %event.wallet,
                    market = %event.market_id,
                    side = %event.side,
                    notional = %event.notional,
                    "Whale trade detected via poller"
                );

                if let Err(e) = trade_tx.send(event).await {
                    tracing::error!(error = %e, "Failed to send whale trade to pipeline");
                }

                total_new_trades += 1;
            }

            // Update last seen timestamp
            if latest_ts > cutoff {
                last_seen.insert(whale.address.clone(), latest_ts);
            }
        }

        if total_new_trades > 0 {
            tracing::info!(
                new_trades = total_new_trades,
                "Whale poller cycle: found {} new trades",
                total_new_trades
            );
        }
    }
}

fn parse_trade_timestamp(ts: Option<&serde_json::Value>) -> Option<DateTime<Utc>> {
    ts.and_then(|t| match t {
        serde_json::Value::Number(n) => {
            let secs = n.as_i64()?;
            // If >1e12, it's milliseconds
            if secs > 1_000_000_000_000 {
                chrono::DateTime::from_timestamp(secs / 1000, ((secs % 1000) * 1_000_000) as u32)
            } else {
                chrono::DateTime::from_timestamp(secs, 0)
            }
        }
        serde_json::Value::String(s) => {
            if let Ok(secs) = s.parse::<i64>() {
                if secs > 1_000_000_000_000 {
                    return chrono::DateTime::from_timestamp(
                        secs / 1000,
                        ((secs % 1000) * 1_000_000) as u32,
                    );
                }
                return chrono::DateTime::from_timestamp(secs, 0);
            }
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        }
        _ => None,
    })
}
