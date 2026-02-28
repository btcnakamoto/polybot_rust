use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::config::AppConfig;
use crate::db::{trade_repo, whale_repo};
use crate::polymarket::DataClient;

/// Run the whale seeder. This is a one-shot task that imports whale wallets
/// from the Polymarket leaderboard if the `whales` table has fewer wallets
/// than the configured minimum.
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

    let entries = match data_client.get_leaderboard(100).await {
        Ok(e) => e,
        Err(e) => {
            tracing::error!(error = %e, "Whale seeder: failed to fetch leaderboard");
            return Err(anyhow::anyhow!("Failed to fetch leaderboard: {e}"));
        }
    };

    // Filter: positive PnL and meaningful volume
    let qualified: Vec<_> = entries
        .iter()
        .enumerate()
        .filter(|(_i, entry)| {
            let pnl = entry.pnl.unwrap_or(Decimal::ZERO);
            let vol = entry.volume.unwrap_or(Decimal::ZERO);
            pnl > Decimal::ZERO && vol > Decimal::from(1_000)
        })
        .collect();

    let max_to_seed = config.basket_max_wallets as usize;
    let to_seed = &qualified[..qualified.len().min(max_to_seed)];

    let mut seeded_count = 0u32;

    for (rank, entry) in to_seed {
        let address = match &entry.address {
            Some(a) if !a.is_empty() => a.clone(),
            _ => continue,
        };

        let label = format!("leaderboard_top_{}", rank + 1);

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

        // Fetch recent trades to seed initial data
        let mut trade_count = 0i32;
        match data_client.get_user_trades(&address, 50).await {
            Ok(trades) => {
                for trade in &trades {
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
                    trades = trades.len(),
                    "Seeded trades for whale"
                );
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    address = %address,
                    "Failed to fetch trades for whale — skipping trade seeding"
                );
            }
        }

        // Update whale stats from leaderboard data + trade count
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
        count = seeded_count,
        "Seeded {} whale wallets from leaderboard",
        seeded_count,
    );

    Ok(())
}
