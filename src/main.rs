use std::sync::Arc;
use tokio::sync::broadcast;

use polybot::api::router::create_router;
use polybot::api::ws_types::WsMessage;
use polybot::config::AppConfig;
use polybot::execution::copy_engine::{self, CopyEngineConfig};
use polybot::execution::order_executor::OrderExecutor;
use polybot::execution::position_sizer::SizingStrategy;
use polybot::execution::risk_manager::RiskLimits;
use polybot::ingestion::pipeline::process_trade_event;
use polybot::ingestion::ws_listener::run_ws_listener;
use polybot::models::{CopySignal, WhaleTradeEvent};
use polybot::polymarket::{ClobClient, DataClient, PolymarketAuth};
use polybot::services::notifier::Notifier;
use polybot::{db, metrics, services, AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = AppConfig::from_env()?;
    let addr = format!("{}:{}", config.host, config.port);

    // --- Prometheus metrics ---
    let metrics_handle = metrics::init_metrics();
    tracing::info!("Prometheus metrics initialized");

    tracing::info!("Connecting to database...");
    let db = db::init_pool(&config.database_url).await?;
    tracing::info!("Database connected");

    // --- Telegram notifier ---
    let notifier: Option<Arc<Notifier>> = if config.notifications_enabled && config.has_telegram() {
        let n = Notifier::new(
            config.telegram_bot_token.clone().unwrap(),
            config.telegram_chat_id.clone().unwrap(),
        );
        tracing::info!("Telegram notifier enabled");
        Some(Arc::new(n))
    } else {
        tracing::info!("Telegram notifications disabled");
        None
    };

    // --- Market resolution poller ---
    {
        let poller_db = db.clone();
        let data_client = DataClient::new(reqwest::Client::new());
        let notifier_clone = notifier.clone();
        tokio::spawn(async move {
            services::resolution::run_resolution_poller(poller_db, data_client, 300, notifier_clone).await;
        });
        tracing::info!("Market resolution poller spawned (interval=300s)");
    }

    // --- Execution layer: copy engine ---
    let (signal_tx, signal_rx) = tokio::sync::mpsc::channel::<CopySignal>(500);

    if config.copy_enabled {
        let clob_client = if config.has_polymarket_auth() {
            let auth = PolymarketAuth::new(
                config.polymarket_api_key.clone().unwrap(),
                config.polymarket_api_secret.clone().unwrap(),
                config.polymarket_passphrase.clone().unwrap(),
            );
            Some(ClobClient::new(reqwest::Client::new(), auth))
        } else {
            tracing::warn!("No Polymarket API credentials — copy engine will run in dry-run mode");
            None
        };

        let engine_config = CopyEngineConfig {
            strategy: SizingStrategy::parse_strategy(&config.copy_strategy),
            bankroll: config.bankroll,
            base_amount: config.base_copy_amount,
            risk_limits: RiskLimits::default(),
        };

        let executor = OrderExecutor::new(clob_client, RiskLimits::default());
        let engine_db = db.clone();
        let engine_notifier = notifier.clone();

        tokio::spawn(async move {
            copy_engine::run_copy_engine(signal_rx, engine_db, executor, engine_config, engine_notifier).await;
        });

        tracing::info!(
            strategy = %config.copy_strategy,
            bankroll = %config.bankroll,
            "Copy engine spawned"
        );
    } else {
        tracing::info!("Copy engine disabled (COPY_ENABLED=false)");
        // Drop the receiver so pipeline doesn't block
        drop(signal_rx);
    }

    // --- Data pipeline: WS ingestion → intelligence → execution ---
    let (ws_tx, mut ws_rx) = tokio::sync::mpsc::channel::<WhaleTradeEvent>(1000);

    if config.ws_subscribe_token_ids.is_empty() {
        tracing::warn!("WS_SUBSCRIBE_TOKEN_IDS is empty — WebSocket listener will not start");
    } else {
        let ws_url = config.polymarket_ws_url.clone();
        let token_ids = config.ws_subscribe_token_ids.clone();
        tracing::info!(
            token_count = token_ids.len(),
            "Starting WebSocket listener"
        );
        tokio::spawn(async move {
            run_ws_listener(ws_url, token_ids, ws_tx).await;
        });

        // Pipeline consumer: intelligence + signal emission
        let pipeline_db = db.clone();
        let copy_enabled = config.copy_enabled;
        let pipeline_notifier = notifier.clone();
        tokio::spawn(async move {
            let signal_sender = if copy_enabled { Some(&signal_tx) } else { None };
            while let Some(event) = ws_rx.recv().await {
                tracing::debug!(
                    wallet = %event.wallet,
                    notional = %event.notional,
                    "WhaleTradeEvent received in pipeline"
                );
                if let Err(e) = process_trade_event(
                    &event,
                    &pipeline_db,
                    signal_sender,
                    pipeline_notifier.as_deref(),
                ).await {
                    tracing::error!(
                        error = %e,
                        wallet = %event.wallet,
                        "Pipeline processing failed"
                    );
                }
            }
            tracing::warn!("WhaleTradeEvent channel closed");
        });
    }

    // --- WebSocket broadcast channel for dashboard ---
    let (ws_broadcast_tx, _) = broadcast::channel::<WsMessage>(256);

    let state = AppState {
        db,
        config,
        ws_tx: ws_broadcast_tx,
        metrics_handle,
        notifier,
    };
    let router = create_router(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Server listening on {addr}");

    // --- Graceful shutdown ---
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Shutting down gracefully...");
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for ctrl+c");
    tracing::info!("Received SIGINT (Ctrl+C), starting graceful shutdown...");
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt::layer())
        .init();
}
