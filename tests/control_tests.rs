mod common;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use polybot::api::router::create_router;
use polybot::api::ws_types::WsMessage;
use polybot::config::AppConfig;
use polybot::AppState;

async fn build_test_app() -> (axum::Router, Arc<AtomicBool>) {
    let pool = common::setup_test_db().await;
    let (ws_tx, _) = tokio::sync::broadcast::channel::<WsMessage>(16);
    let metrics_handle = polybot::metrics::init_metrics();
    let pause_flag = Arc::new(AtomicBool::new(false));

    let config = AppConfig::from_env().unwrap_or_else(|_| AppConfig {
        database_url: std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://polybot:password@localhost:5432/polybot_test".into()),
        host: "127.0.0.1".into(),
        port: 0,
        redis_url: None,
        polymarket_api_key: None,
        polymarket_api_secret: None,
        polymarket_passphrase: None,
        polymarket_ws_url: "wss://localhost".into(),
        ws_subscribe_token_ids: vec![],
        private_key: None,
        polygon_rpc_url: "https://polygon-rpc.com".into(),
        dry_run: true,
        copy_strategy: "fixed".into(),
        bankroll: rust_decimal::Decimal::from(1000),
        base_copy_amount: rust_decimal::Decimal::from(50),
        copy_enabled: false,
        telegram_bot_token: None,
        telegram_chat_id: None,
        notifications_enabled: false,
        basket_consensus_threshold: rust_decimal::Decimal::new(80, 2),
        basket_time_window_hours: 48,
        basket_min_wallets: 5,
        basket_max_wallets: 10,
        basket_enabled: false,
        market_discovery_enabled: false,
        market_discovery_interval_secs: 300,
        market_min_volume: rust_decimal::Decimal::from(10_000),
        market_min_liquidity: rust_decimal::Decimal::from(5_000),
        whale_seeder_enabled: false,
        whale_seeder_skip_top_n: 10,
        whale_seeder_min_trades: 100,
        whale_poller_interval_secs: 60,
        chain_listener_enabled: false,
        polygon_ws_url: None,
        default_stop_loss_pct: rust_decimal::Decimal::new(1500, 2),
        default_take_profit_pct: rust_decimal::Decimal::new(5000, 2),
        position_monitor_interval_secs: 30,
        tracked_whale_min_notional: rust_decimal::Decimal::from(500),
        min_resolved_for_signal: 5,
        min_signal_win_rate: rust_decimal::Decimal::new(60, 2),
        min_total_trades_for_signal: 50,
        min_signal_notional: rust_decimal::Decimal::from(50_000),
        max_signal_notional: rust_decimal::Decimal::from(500_000),
        min_signal_ev: rust_decimal::Decimal::from(50),
        assumed_slippage_pct: rust_decimal::Decimal::new(2, 2),
    });

    let state = AppState {
        db: pool,
        config,
        ws_tx,
        metrics_handle,
        notifier: None,
        wallet: None,
        trading_client: None,
        balance_checker: None,
        pause_flag: Arc::clone(&pause_flag),
    };

    let router = create_router(state);
    (router, pause_flag)
}

#[tokio::test]
async fn test_control_stop() {
    let (app, pause_flag) = build_test_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/control/stop")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "paused");

    // Verify the pause flag was actually set
    assert!(pause_flag.load(std::sync::atomic::Ordering::Relaxed));
}

#[tokio::test]
async fn test_control_resume() {
    let (app, pause_flag) = build_test_app().await;

    // First pause
    pause_flag.store(true, std::sync::atomic::Ordering::Relaxed);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/control/resume")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "running");

    // Verify the pause flag was cleared
    assert!(!pause_flag.load(std::sync::atomic::Ordering::Relaxed));
}

#[tokio::test]
async fn test_control_status() {
    let (app, _pause_flag) = build_test_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/control/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // In test setup: dry_run=true, no wallet
    assert_eq!(json["mode"], "dry_run");
    assert_eq!(json["paused"], false);
    assert!(json["wallet"].is_null());
    assert!(json["usdc_balance"].is_null());
    assert_eq!(json["copy_enabled"], false);
}

#[tokio::test]
async fn test_control_cancel_all_no_wallet() {
    let (app, _pause_flag) = build_test_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/control/cancel-all")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 400 since no trading client is configured
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["error"].as_str().unwrap().contains("monitor-only"));
}
