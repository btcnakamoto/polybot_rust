mod common;

use chrono::Utc;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::time::Instant;

use polybot::db::{whale_repo, trade_repo};
use polybot::ingestion::pipeline::{process_trade_event, PipelineConfig};
use polybot::models::{Side, WhaleTradeEvent};

fn default_pipeline_config() -> PipelineConfig {
    PipelineConfig {
        tracked_whale_min_notional: Decimal::from(500),
        min_signal_win_rate: Decimal::new(60, 2),
        min_resolved_for_signal: 5,
        min_total_trades_for_signal: 100,
        min_signal_notional: Decimal::from(50_000),
        max_signal_notional: Decimal::from(500_000),
        min_signal_ev: Decimal::from(50),
        assumed_slippage_pct: Decimal::new(2, 2),
        signal_dedup_window_secs: 10,
    }
}

fn make_trade_event(wallet: &str, notional: i64, side: Side) -> WhaleTradeEvent {
    WhaleTradeEvent {
        wallet: wallet.into(),
        market_id: "market_test_001".into(),
        asset_id: "token_test_001".into(),
        side,
        size: Decimal::from(100),
        price: Decimal::new(65, 2), // 0.65
        notional: Decimal::from(notional),
        timestamp: Utc::now(),
    }
}

#[tokio::test]
async fn test_large_trade_creates_whale_and_records_trade() {
    let pool = common::setup_test_db().await;
    let config = default_pipeline_config();
    let dedup = tokio::sync::Mutex::new(HashMap::<String, Instant>::new());

    let event = make_trade_event("0xWHALE_LARGE_001", 50_000, Side::Buy);

    process_trade_event(&event, &pool, None, None, &config, &dedup)
        .await
        .expect("Pipeline should succeed");

    // Verify whale was created
    let whale = whale_repo::get_whale_by_address(&pool, "0xWHALE_LARGE_001")
        .await
        .expect("DB query should succeed")
        .expect("Whale should exist");

    assert_eq!(whale.address, "0xWHALE_LARGE_001");

    // Verify trade was recorded
    let trades = trade_repo::get_trades_by_whale(&pool, whale.id)
        .await
        .expect("DB query should succeed");

    assert_eq!(trades.len(), 1);
    assert_eq!(trades[0].market_id, "market_test_001");
    assert_eq!(trades[0].notional, Decimal::from(50_000));
}

#[tokio::test]
async fn test_small_trade_is_filtered() {
    let pool = common::setup_test_db().await;
    let config = default_pipeline_config();
    let dedup = tokio::sync::Mutex::new(HashMap::<String, Instant>::new());

    let event = make_trade_event("0xWHALE_SMALL_001", 500, Side::Buy);

    process_trade_event(&event, &pool, None, None, &config, &dedup)
        .await
        .expect("Pipeline should succeed");

    // Whale should NOT be created for small trades
    let whale = whale_repo::get_whale_by_address(&pool, "0xWHALE_SMALL_001")
        .await
        .expect("DB query should succeed");

    assert!(whale.is_none(), "Small trades should be filtered");
}

#[tokio::test]
async fn test_classification_updates_on_multiple_trades() {
    let pool = common::setup_test_db().await;
    let config = default_pipeline_config();
    let dedup = tokio::sync::Mutex::new(HashMap::<String, Instant>::new());

    // Send multiple trades from the same wallet
    for i in 0..5 {
        let event = WhaleTradeEvent {
            wallet: "0xWHALE_CLASSIFY_001".into(),
            market_id: format!("market_classify_{}", i),
            asset_id: format!("token_classify_{}", i),
            side: Side::Buy,
            size: Decimal::from(100),
            price: Decimal::new(60, 2),
            notional: Decimal::from(20_000),
            timestamp: Utc::now(),
        };

        process_trade_event(&event, &pool, None, None, &config, &dedup)
            .await
            .expect("Pipeline should succeed");
    }

    let whale = whale_repo::get_whale_by_address(&pool, "0xWHALE_CLASSIFY_001")
        .await
        .expect("DB query should succeed")
        .expect("Whale should exist");

    // Classification should be set (default for directional trader is "informed")
    assert!(
        whale.classification.is_some(),
        "Classification should be set after multiple trades"
    );

    // Verify trades count
    let trades = trade_repo::get_trades_by_whale(&pool, whale.id)
        .await
        .expect("DB query should succeed");

    assert_eq!(trades.len(), 5);
}
