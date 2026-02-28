use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::config::AppConfig;
use crate::db::{trade_repo, whale_repo};
use crate::polymarket::DataClient;

/// Run the whale seeder. This is a one-shot task that imports whale wallets
/// from the Polymarket leaderboard if the `whales` table has fewer wallets
/// than the configured minimum.
///
/// Anti-signal filtering (from README):
/// - Skip top N leaderboard wallets (everyone copies them — edge is gone)
/// - Require >= min_trades historical trades (small sample = luck, not skill)
/// - Positive PnL + meaningful volume
pub async fn run_whale_seeder(
    data_client: &DataClient,
    pool: &PgPool,
    config: &AppConfig,
) -> anyhow::Result<()> {
    // Check how many active whales we already have
    let existing = whale_repo::get_active_whales(pool).await?;
    if existing.len() as i32 >= config.basket_min_wallets {
        tracing::info!(
            existing = existing.len(),
            min = config.basket_min_wallets,
            "Whale seeder: enough wallets already tracked, skipping"
        );
        return Ok(());
    }

    tracing::info!("Whale seeder: fetching leaderboard to seed wallets");

    // Fetch more entries to account for filtering
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

    tracing::info!(
        total_entries = entries.len(),
        skip_top_n = skip_top_n,
        min_trades = min_trades,
        "Anti-signal filtering: skipping top {} leaderboard, requiring >= {} trades",
        skip_top_n,
        min_trades,
    );

    // Anti-signal filter 1: Skip top N leaderboard wallets
    // README: "Don't copy the top leaderboard accounts — everyone already copies them"
    let filtered_entries: Vec<_> = entries
        .iter()
        .enumerate()
        .filter(|(rank, entry)| {
            // Skip top N (0-indexed)
            if *rank < skip_top_n {
                if let Some(addr) = &entry.address {
                    tracing::debug!(
                        rank = rank + 1,
                        address = %addr,
                        "Anti-signal: skipping top leaderboard wallet (edge is gone)"
                    );
                }
                return false;
            }

            // Must have positive PnL
            let pnl = entry.pnl.unwrap_or(Decimal::ZERO);
            if pnl <= Decimal::ZERO {
                return false;
            }

            // Must have meaningful volume
            let vol = entry.volume.unwrap_or(Decimal::ZERO);
            if vol <= Decimal::from(1_000) {
                return false;
            }

            true
        })
        .collect();

    let max_to_seed = config.basket_max_wallets as usize;
    let mut seeded_count = 0u32;
    let mut skipped_low_trades = 0u32;

    for (rank, entry) in &filtered_entries {
        if seeded_count as usize >= max_to_seed {
            break;
        }

        let address = match &entry.address {
            Some(a) if !a.is_empty() => a.clone(),
            _ => continue,
        };

        // Anti-signal filter 2: Check trade count before seeding
        // README: "Don't copy wallets with <100 trades — small sample size"
        let user_trades = match data_client.get_user_trades(&address, 200).await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    address = %address,
                    "Failed to fetch trades for trade-count check — skipping"
                );
                continue;
            }
        };

        if (user_trades.len() as u32) < min_trades {
            tracing::debug!(
                address = %address,
                trade_count = user_trades.len(),
                min_required = min_trades,
                "Anti-signal: skipping wallet with too few trades (can't distinguish skill from luck)"
            );
            skipped_low_trades += 1;
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

        // Update label via direct query
        if let Err(e) = sqlx::query("UPDATE whales SET label = $2 WHERE id = $1")
            .bind(whale.id)
            .bind(&label)
            .execute(pool)
            .await
        {
            tracing::warn!(error = %e, "Failed to update whale label");
        }

        // Seed trades from the already-fetched user_trades
        let mut trade_count = 0i32;
        for trade in &user_trades {
            let token_id = trade.token_id.as_deref().unwrap_or("unknown");
            let market_id = trade.market.as_deref().unwrap_or("unknown");
            let side = trade.side.as_deref().unwrap_or("BUY");
            let size = trade.size.unwrap_or(Decimal::ZERO);
            let price = trade.price.unwrap_or(Decimal::ZERO);
            let notional = size * price;

            let traded_at = trade
                .timestamp
                .as_ref()
                .and_then(|t| match t {
                    serde_json::Value::Number(n) => {
                        n.as_i64().and_then(|secs| chrono::DateTime::from_timestamp(secs, 0))
                    }
                    serde_json::Value::String(s) => {
                        if let Ok(secs) = s.parse::<i64>() {
                            return chrono::DateTime::from_timestamp(secs, 0);
                        }
                        chrono::DateTime::parse_from_rfc3339(s)
                            .ok()
                            .map(|dt| dt.with_timezone(&Utc))
                    }
                    _ => None,
                })
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

        tracing::debug!(
            address = %address,
            trades = user_trades.len(),
            inserted = trade_count,
            "Seeded trades for whale"
        );

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

        if let Err(e) = sqlx::query(
            r#"UPDATE whales
               SET total_pnl = $2, total_trades = $3, classification = $4,
                   category = $5, updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(whale.id)
        .bind(pnl)
        .bind(trade_count)
        .bind(classification)
        .bind(format!("vol:{}", vol.round()))
        .execute(pool)
        .await
        {
            tracing::warn!(error = %e, "Failed to update whale stats from leaderboard");
        }

        seeded_count += 1;
    }

    tracing::info!(
        seeded = seeded_count,
        skipped_top_n = skip_top_n,
        skipped_low_trades = skipped_low_trades,
        "Whale seeder complete: seeded {} wallets (skipped top {} as anti-signal, {} had <{} trades)",
        seeded_count,
        skip_top_n,
        skipped_low_trades,
        min_trades,
    );

    Ok(())
}
