use axum::middleware;
use axum::routing::{delete, get, post};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::AppState;
use super::auth::require_auth;
use super::handlers;

pub fn create_router(state: AppState) -> Router {
    // Public routes — no authentication required
    let public = Router::new()
        .route("/health", get(handlers::health::health_check))
        .route("/metrics", get(handlers::metrics::render));

    // Protected API routes — require Bearer token when API_TOKEN is set
    let protected = Router::new()
        // Dashboard
        .route("/api/dashboard/summary", get(handlers::dashboard::summary))
        // Whales
        .route("/api/whales", get(handlers::whales::list))
        .route("/api/whales/:address", get(handlers::whales::detail))
        .route("/api/whales/:id/trades", get(handlers::whales::trades))
        // Trades (copy orders)
        .route("/api/trades", get(handlers::trades::list))
        // Positions
        .route("/api/positions", get(handlers::positions::list))
        .route("/api/positions/:id/close", post(handlers::positions::close))
        // Baskets
        .route("/api/baskets", get(handlers::baskets::list).post(handlers::baskets::create))
        .route("/api/baskets/:id", get(handlers::baskets::detail))
        .route("/api/baskets/:id/whales", get(handlers::baskets::whales).post(handlers::baskets::add_whale))
        .route("/api/baskets/:id/whales/:whale_id", delete(handlers::baskets::remove_whale))
        .route("/api/baskets/:id/consensus", get(handlers::baskets::consensus_history))
        .route("/api/consensus/recent", get(handlers::baskets::recent_consensus))
        // Analytics
        .route("/api/analytics/pnl-history", get(handlers::analytics::pnl_history))
        .route("/api/analytics/performance", get(handlers::analytics::performance))
        // Config
        .route("/api/config", get(handlers::config::get_config).put(handlers::config::update_config))
        // Control
        .route("/api/control/stop", post(handlers::control::stop))
        .route("/api/control/resume", post(handlers::control::resume))
        .route("/api/control/status", get(handlers::control::status))
        .route("/api/control/cancel-all", post(handlers::control::cancel_all))
        // WebSocket
        .route("/ws", get(handlers::ws::handler))
        .layer(middleware::from_fn(require_auth));

    // CORS: allow same-origin + common dashboard origins
    let cors = CorsLayer::new()
        .allow_origin(Any) // nginx proxies from same origin; direct API access needs token
        .allow_methods(Any)
        .allow_headers(Any);

    public
        .merge(protected)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
