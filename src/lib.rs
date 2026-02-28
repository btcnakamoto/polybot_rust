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

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::api::ws_types::WsMessage;
use crate::config::AppConfig;
use crate::polymarket::wallet::PolymarketWallet;
use crate::polymarket::trading::TradingClient;
use crate::polymarket::balance::BalanceChecker;
use crate::services::notifier::Notifier;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub config: AppConfig,
    pub ws_tx: broadcast::Sender<WsMessage>,
    pub metrics_handle: metrics_exporter_prometheus::PrometheusHandle,
    pub notifier: Option<Arc<Notifier>>,
    pub wallet: Option<Arc<PolymarketWallet>>,
    pub trading_client: Option<Arc<TradingClient>>,
    pub balance_checker: Option<Arc<BalanceChecker>>,
    /// Global pause flag — when true, copy engine skips all signals.
    pub pause_flag: Arc<AtomicBool>,
}
