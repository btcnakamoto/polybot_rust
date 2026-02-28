use rust_decimal::Decimal;
use sqlx::PgPool;
use tokio::sync::mpsc;

use crate::db::{trade_repo, whale_repo};
use crate::intelligence::classifier::Classification;
use crate::intelligence::{classify_wallet, score_wallet};
use crate::models::{CopySignal, TradeResult, WhaleTradeEvent};

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
) -> anyhow::Result<()> {
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

    // Update last_trade_at
    whale_repo::touch_whale_last_trade(pool, whale.id, event.timestamp).await?;

    // Step 4: Fetch trade history and re-score
    let all_trades = trade_repo::get_trades_by_whale(pool, whale.id).await?;

    // Classify wallet
    let classification = classify_wallet(&all_trades);
    whale_repo::update_whale_classification(pool, whale.id, classification.as_str()).await?;

    // Score wallet (convert DB trades to TradeResults for scoring)
    // Note: simplified profit model — will be refined with market resolution tracking
    let trade_results: Vec<TradeResult> = all_trades
        .iter()
        .map(|t| {
            let profit = if t.side == "BUY" {
                t.notional * Decimal::new(10, 2) // placeholder +10%
            } else {
                t.notional * Decimal::new(-5, 2) // placeholder -5%
            };
            TradeResult {
                profit,
                traded_at: t.traded_at,
            }
        })
        .collect();

    if trade_results.is_empty() {
        return Ok(());
    }

    let score = score_wallet(&trade_results);

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
                tracing::info!(
                    wallet = %event.wallet,
                    market = %event.market_id,
                    "CopySignal emitted to execution layer"
                );
            }
        }
    }

    Ok(())
}
