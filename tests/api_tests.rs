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

async fn build_test_app() -> (axum::Router, sqlx::PgPool) {
    let pool = common::setup_test_db().await;
    let (ws_tx, _) = tokio::sync::broadcast::channel::<WsMessage>(16);
    let metrics_handle = polybot::metrics::init_metrics();

    let config = AppConfig::from_env().unwrap_or_else(|_| {
        // Minimal config for tests
        AppConfig {
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
        }
    });

    let state = AppState {
        db: pool.clone(),
        config,
        ws_tx,
        metrics_handle,
        notifier: None,
        wallet: None,
        trading_client: None,
        balance_checker: None,
        pause_flag: Arc::new(AtomicBool::new(false)),
    };

    let router = create_router(state);
    (router, pool)
}

#[tokio::test]
async fn test_health_check() {
    let (app, _pool) = build_test_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "healthy");
}

#[tokio::test]
async fn test_get_whales() {
    let (app, _pool) = build_test_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/whales")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["success"], true);
    assert!(json["data"].is_array());
}

#[tokio::test]
async fn test_dashboard_summary() {
    let (app, _pool) = build_test_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/dashboard/summary")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["tracked_whales"].is_number());
    assert!(json["open_positions"].is_number());
    assert!(json["total_pnl"].is_string());
    assert!(json["today_pnl"].is_string());
    assert!(json["active_baskets"].is_number());
    assert!(json["recent_consensus_count"].is_number());
}

#[tokio::test]
async fn test_create_and_list_baskets() {
    let (app, _pool) = build_test_app().await;

    // Create basket
    let create_body = serde_json::json!({
        "name": "test_basket_api",
        "category": "crypto",
    });

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/baskets")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&create_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["name"], "test_basket_api");

    // List baskets
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/baskets")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["success"], true);
    let baskets = json["data"].as_array().unwrap();
    assert!(baskets.iter().any(|b| b["name"] == "test_basket_api"));
}

#[tokio::test]
async fn test_metrics_endpoint() {
    let (app, _pool) = build_test_app().await;

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let _text = String::from_utf8(body.to_vec()).unwrap();
    // Endpoint returns valid text; metric names may or may not appear depending
    // on global recorder state in tests (only one recorder per process).
}
