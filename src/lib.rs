pub mod api;
pub mod config;
pub mod db;
pub mod errors;
pub mod metrics;
pub mod models;
pub mod ingestion;
pub mod intelligence;
pub mod execution;
pub mod polymarket;
pub mod services;

use std::sync::Arc;
use tokio::sync::broadcast;

use crate::api::ws_types::WsMessage;
use crate::config::AppConfig;
use crate::services::notifier::Notifier;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub config: AppConfig,
    pub ws_tx: broadcast::Sender<WsMessage>,
    pub metrics_handle: metrics_exporter_prometheus::PrometheusHandle,
    pub notifier: Option<Arc<Notifier>>,
}
