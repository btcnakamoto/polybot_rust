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
