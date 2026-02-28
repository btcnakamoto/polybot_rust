mod api;
mod config;
mod db;
mod errors;
mod models;
mod ingestion;
mod intelligence;
mod execution;
mod polymarket;

use tokio::sync::broadcast;

use crate::api::router::create_router;
use crate::api::ws_types::WsMessage;
use crate::config::AppConfig;
use crate::execution::copy_engine::{self, CopyEngineConfig};
use crate::execution::order_executor::OrderExecutor;
use crate::execution::position_sizer::SizingStrategy;
use crate::execution::risk_manager::RiskLimits;
use crate::ingestion::pipeline::process_trade_event;
use crate::ingestion::ws_listener::run_ws_listener;
use crate::models::{CopySignal, WhaleTradeEvent};
use crate::polymarket::{ClobClient, PolymarketAuth};

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub config: AppConfig,
    pub ws_tx: broadcast::Sender<WsMessage>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = AppConfig::from_env()?;
    let addr = format!("{}:{}", config.host, config.port);

    tracing::info!("Connecting to database...");
    let db = db::init_pool(&config.database_url).await?;
    tracing::info!("Database connected");

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
            strategy: SizingStrategy::from_str(&config.copy_strategy),
            bankroll: config.bankroll,
            base_amount: config.base_copy_amount,
            risk_limits: RiskLimits::default(),
        };

        let executor = OrderExecutor::new(clob_client, RiskLimits::default());
        let engine_db = db.clone();

        tokio::spawn(async move {
            copy_engine::run_copy_engine(signal_rx, engine_db, executor, engine_config).await;
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
        tokio::spawn(async move {
            let signal_sender = if copy_enabled { Some(&signal_tx) } else { None };
            while let Some(event) = ws_rx.recv().await {
                tracing::debug!(
                    wallet = %event.wallet,
                    notional = %event.notional,
                    "WhaleTradeEvent received in pipeline"
                );
                if let Err(e) = process_trade_event(&event, &pipeline_db, signal_sender).await {
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
    };
    let router = create_router(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Server listening on {addr}");
    axum::serve(listener, router).await?;

    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt::layer())
        .init();
}
