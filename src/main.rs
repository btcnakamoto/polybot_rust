mod api;
mod config;
mod db;
mod errors;
mod models;
mod ingestion;
mod intelligence;
mod execution;
mod polymarket;

use crate::api::router::create_router;
use crate::config::AppConfig;
use crate::ingestion::pipeline::process_trade_event;
use crate::ingestion::ws_listener::run_ws_listener;
use crate::models::WhaleTradeEvent;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub config: AppConfig,
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

    // --- Data pipeline: WS ingestion → intelligence processing ---
    let (tx, mut rx) = tokio::sync::mpsc::channel::<WhaleTradeEvent>(1000);

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
            run_ws_listener(ws_url, token_ids, tx).await;
        });

        // Phase 2: Intelligence pipeline consumer
        let pipeline_db = db.clone();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                tracing::debug!(
                    wallet = %event.wallet,
                    notional = %event.notional,
                    "WhaleTradeEvent received in pipeline"
                );
                if let Err(e) = process_trade_event(&event, &pipeline_db).await {
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

    let state = AppState { db, config };
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
