use std::collections::HashSet;

use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::config::AppConfig;
use crate::db::{trade_repo, whale_repo};
use crate::polymarket::data_client::UserTrade;
use crate::polymarket::DataClient;

/// Maximum number of days since last trade to consider a whale "active".
/// Stale-deactivation uses this threshold; seeder discovery uses a more
/// lenient window (SEEDER_RECENCY_DAYS) since the API only returns 200 trades.
const MAX_INACTIVE_DAYS: i64 = 30;
const SEEDER_RECENCY_DAYS: i64 = 90;

/// Run the whale seeder periodically. Discovers new whales from the Polymarket
/// leaderboard and deactivates stale ones that haven't traded recently.
///
/// Anti-signal filtering (from README):
/// - Skip top N leaderboard wallets (everyone copies them — edge is gone)
/// - Require >= min_trades historical trades (small sample = luck, not skill)
/// - Positive PnL + meaningful volume
/// - Must have traded within the last 30 days (recency filter)
pub async fn run_whale_seeder_loop(
    data_client: DataClient,
    pool: PgPool,
    config: AppConfig,
    interval_secs: u64,
) {
    // Run immediately on startup
    if let Err(e) = seed_and_cleanup(&data_client, &pool, &config).await {
        tracing::warn!(error = %e, "Whale seeder initial run failed (non-fatal)");
    }

    // Then run periodically
    let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
    ticker.tick().await; // skip first immediate tick

    loop {
        ticker.tick().await;
        if let Err(e) = seed_and_cleanup(&data_client, &pool, &config).await {
            tracing::warn!(error = %e, "Whale seeder periodic run failed (non-fatal)");
        }
    }
}

/// One-shot seeder (kept for backward compat / tests).
pub async fn run_whale_seeder(
    data_client: &DataClient,
    pool: &PgPool,
    config: &AppConfig,
) -> anyhow::Result<()> {
    seed_and_cleanup(data_client, pool, config).await
}

/// Core logic: deactivate stale whales, then discover new ones.
async fn seed_and_cleanup(
    data_client: &DataClient,
    pool: &PgPool,
    config: &AppConfig,
) -> anyhow::Result<()> {
    // Step 1: Deactivate whales that haven't traded in MAX_INACTIVE_DAYS
    let deactivated = whale_repo::deactivate_stale_whales(pool, MAX_INACTIVE_DAYS).await?;
    if deactivated > 0 {
        tracing::info!(
            count = deactivated,
            days = MAX_INACTIVE_DAYS,
            "Auto-deactivated {} stale whales (no trades in {} days)",
            deactivated,
            MAX_INACTIVE_DAYS,
        );
    }

    // Step 2: Check if we need more active whales
    let active = whale_repo::get_active_whales(pool).await?;
    let max_wallets = config.basket_max_wallets as usize;

    if active.len() >= max_wallets {
        tracing::debug!(
            active = active.len(),
            max = max_wallets,
            "Whale seeder: at capacity, skipping discovery"
        );
        return Ok(());
    }

    let slots_available = max_wallets - active.len();
    tracing::info!(
        active = active.len(),
        slots = slots_available,
        "Whale seeder: {} slots available, discovering new whales",
        slots_available,
    );

    // Step 3: Fetch leaderboard and seed new whales
    let fetch_count = 200u32;
    let entries = match data_client.get_leaderboard(fetch_count).await {
        Ok(e) => e,
        Err(e) => {
            tracing::error!(error = %e, "Whale seeder: failed to fetch leaderboard");
            return Err(anyhow::anyhow!("Failed to fetch leaderboard: {e}"));
        }
    };

    let skip_top_n = config.whale_seeder_skip_top_n;
    let min_trades = config.whale_seeder_min_trades;

    // Build set of already-tracked addresses to avoid re-processing
    let tracked_addrs: std::collections::HashSet<String> = whale_repo::get_all_whale_addresses(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();

    // Anti-signal filter 1: Skip top N, require positive PnL, meaningful volume
    let filtered_entries: Vec<_> = entries
        .iter()
        .enumerate()
        .filter(|(rank, entry)| {
            if *rank < skip_top_n {
                return false;
            }
            let pnl = entry.pnl.unwrap_or(Decimal::ZERO);
            if pnl <= Decimal::ZERO {
                return false;
            }
            let vol = entry.volume.unwrap_or(Decimal::ZERO);
            if vol <= Decimal::from(1_000) {
                return false;
            }
            true
        })
        .collect();

    let mut seeded_count = 0u32;
    let mut skipped_inactive = 0u32;
    let mut skipped_low_trades = 0u32;
    let mut skipped_bot_mm = 0u32;

    for (rank, entry) in &filtered_entries {
        if seeded_count as usize >= slots_available {
            break;
        }

        let address = match &entry.address {
            Some(a) if !a.is_empty() => a.clone(),
            _ => continue,
        };

        // Skip already-tracked whales
        if tracked_addrs.contains(&address) {
            continue;
        }

        // Fetch recent trades for this wallet
        let user_trades = match data_client.get_user_trades(&address, 200).await {
            Ok(t) => t,
            Err(e) => {
                tracing::debug!(error = %e, address = %address, "Failed to fetch trades — skipping");
                continue;
            }
        };

        // Anti-signal filter 2: Minimum trade count
        if (user_trades.len() as u32) < min_trades {
            skipped_low_trades += 1;
            continue;
        }

        // Anti-signal filter 3: Recency — most recent trade must be within SEEDER_RECENCY_DAYS.
        // Note: stale-deactivation (MAX_INACTIVE_DAYS=30) will later prune whales that go
        // quiet, so a wider discovery window here is safe.
        let most_recent_trade = user_trades
            .iter()
            .filter_map(|t| parse_trade_timestamp(t.timestamp.as_ref()))
            .max();

        match most_recent_trade {
            Some(latest) => {
                let days_since = (Utc::now() - latest).num_days();
                if days_since > SEEDER_RECENCY_DAYS {
                    tracing::debug!(
                        address = %address,
                        days_since = days_since,
                        "Skipping inactive whale (last trade {} days ago)",
                        days_since,
                    );
                    skipped_inactive += 1;
                    continue;
                }
            }
            None => {
                // No parseable timestamps — skip
                skipped_inactive += 1;
                continue;
            }
        }

        // Anti-signal filter 4: Bot/MM detection from trade patterns
        if let Some(reason) = detect_bot_or_mm(&user_trades) {
            tracing::info!(
                address = %address,
                reason = %reason,
                "Skipping suspected bot/MM whale"
            );
            skipped_bot_mm += 1;
            continue;
        }

        let label = format!("leaderboard_rank_{}", rank + 1);

        // Upsert whale
        let whale = match whale_repo::upsert_whale(pool, &address).await {
            Ok(w) => w,
            Err(e) => {
                tracing::warn!(error = %e, address = %address, "Failed to upsert whale");
                continue;
            }
        };

        // Seed trades
        let mut trade_count = 0i32;
        for trade in &user_trades {
            let token_id = trade.token_id.as_deref().unwrap_or("unknown");
            let market_id = trade.market.as_deref().unwrap_or("unknown");
            let side = trade.side.as_deref().unwrap_or("BUY");
            let size = trade.size.unwrap_or(Decimal::ZERO);
            let price = trade.price.unwrap_or(Decimal::ZERO);
            let notional = size * price;

            let traded_at = parse_trade_timestamp(trade.timestamp.as_ref())
                .unwrap_or_else(Utc::now);

            if let Err(e) = trade_repo::insert_trade(
                pool, whale.id, market_id, token_id, side, size, price, notional, traded_at,
            )
            .await
            {
                tracing::debug!(error = %e, "Failed to insert seeded trade (may be duplicate)");
            } else {
                trade_count += 1;
            }
        }

        // Update whale stats from leaderboard data
        let pnl = entry.pnl.unwrap_or(Decimal::ZERO);
        let vol = entry.volume.unwrap_or(Decimal::ZERO);
        let classification = if pnl > Decimal::from(100_000) {
            "top_tier"
        } else if pnl > Decimal::from(10_000) {
            "high_performer"
        } else {
            "profitable"
        };

        let _ = sqlx::query(
            r#"UPDATE whales
               SET classification = $2, category = $3, label = $4, updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(whale.id)
        .bind(classification)
        .bind(format!("vol:{}", vol.round()))
        .bind(&label)
        .execute(pool)
        .await;

        // Compute and store initial scores
        let est_win_rate = if pnl > Decimal::from(100_000) {
            Decimal::new(68, 2)
        } else if pnl > Decimal::from(10_000) {
            Decimal::new(63, 2)
        } else {
            Decimal::new(58, 2)
        };
        let est_kelly = est_win_rate * Decimal::from(2) - Decimal::ONE;
        let est_ev = if trade_count > 0 {
            pnl / Decimal::from(trade_count)
        } else {
            Decimal::ZERO
        };
        let est_sharpe = if vol > Decimal::ZERO {
            (pnl / vol * Decimal::from(100)).min(Decimal::from(5))
        } else {
            Decimal::ONE
        };

        let _ = whale_repo::update_whale_scores(
            pool, whale.id, est_sharpe, est_win_rate, est_kelly, est_ev, trade_count, pnl,
        )
        .await;

        tracing::info!(
            address = %address,
            pnl = %pnl,
            trades = trade_count,
            "Seeded new whale"
        );

        seeded_count += 1;
    }

    tracing::info!(
        seeded = seeded_count,
        skipped_inactive = skipped_inactive,
        skipped_low_trades = skipped_low_trades,
        skipped_bot_mm = skipped_bot_mm,
        "Whale seeder cycle complete",
    );

    Ok(())
}

/// Detect bot or market-maker patterns from API trade data.
/// Returns `Some(reason)` if the wallet should be skipped.
fn detect_bot_or_mm(trades: &[UserTrade]) -> Option<String> {
    if trades.len() < 20 {
        return None;
    }

    // Parse timestamps to compute trading frequency
    let timestamps: Vec<_> = trades
        .iter()
        .filter_map(|t| parse_trade_timestamp(t.timestamp.as_ref()))
        .collect();

    if timestamps.len() >= 20 {
        let oldest = *timestamps.iter().min().unwrap();
        let newest = *timestamps.iter().max().unwrap();
        let span_days = (newest - oldest).num_days().max(1);

        // Bot detection: if 200 trades span fewer than 7 days → >28 trades/day average
        if trades.len() >= 100 && span_days < 7 {
            return Some(format!(
                "bot: {} trades in {} days ({:.0} trades/day)",
                trades.len(),
                span_days,
                trades.len() as f64 / span_days as f64,
            ));
        }

        // Also catch high frequency over longer spans: >50 trades/day
        let trades_per_day = trades.len() as f64 / span_days as f64;
        if trades_per_day > 50.0 {
            return Some(format!(
                "bot: {:.0} trades/day over {} days",
                trades_per_day, span_days,
            ));
        }
    }

    // Market-maker detection: >40% of markets have both BUY and SELL
    let mut market_buy: HashSet<String> = HashSet::new();
    let mut market_sell: HashSet<String> = HashSet::new();

    for trade in trades {
        let market = trade.market.as_deref().unwrap_or("").to_string();
        if market.is_empty() {
            continue;
        }
        match trade.side.as_deref().unwrap_or("").to_uppercase().as_str() {
            "BUY" => { market_buy.insert(market); }
            "SELL" => { market_sell.insert(market); }
            _ => {}
        }
    }

    let dual_side = market_buy.intersection(&market_sell).count();
    let total_markets = market_buy.union(&market_sell).count();

    if total_markets >= 5 {
        let dual_ratio = dual_side as f64 / total_markets as f64;
        if dual_ratio > 0.40 {
            return Some(format!(
                "market_maker: {}/{} markets ({:.0}%) have dual-side activity",
                dual_side, total_markets, dual_ratio * 100.0,
            ));
        }
    }

    None
}

fn parse_trade_timestamp(ts: Option<&serde_json::Value>) -> Option<chrono::DateTime<Utc>> {
    ts.and_then(|t| match t {
        serde_json::Value::Number(n) => {
            let secs = n.as_i64()?;
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
