use axum::routing::{delete, get};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::AppState;
use super::handlers;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Health
        .route("/health", get(handlers::health::health_check))
        // Dashboard
        .route("/api/dashboard/summary", get(handlers::dashboard::summary))
        // Whales
        .route("/api/whales", get(handlers::whales::list))
        .route("/api/whales/{address}", get(handlers::whales::detail))
        .route("/api/whales/{id}/trades", get(handlers::whales::trades))
        // Trades (copy orders)
        .route("/api/trades", get(handlers::trades::list))
        // Positions
        .route("/api/positions", get(handlers::positions::list))
        // Baskets
        .route("/api/baskets", get(handlers::baskets::list).post(handlers::baskets::create))
        .route("/api/baskets/{id}", get(handlers::baskets::detail))
        .route("/api/baskets/{id}/whales", get(handlers::baskets::whales).post(handlers::baskets::add_whale))
        .route("/api/baskets/{id}/whales/{whale_id}", delete(handlers::baskets::remove_whale))
        .route("/api/baskets/{id}/consensus", get(handlers::baskets::consensus_history))
        .route("/api/consensus/recent", get(handlers::baskets::recent_consensus))
        // WebSocket
        .route("/ws", get(handlers::ws::handler))
        // Middleware
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
