use std::sync::atomic::Ordering;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

use crate::AppState;

/// POST /api/control/stop — Pause the copy engine.
pub async fn stop(State(state): State<AppState>) -> impl IntoResponse {
    state.pause_flag.store(true, Ordering::Relaxed);
    tracing::warn!("Copy engine PAUSED via control API");
    (StatusCode::OK, Json(json!({ "status": "paused" })))
}

/// POST /api/control/resume — Resume the copy engine.
pub async fn resume(State(state): State<AppState>) -> impl IntoResponse {
    state.pause_flag.store(false, Ordering::Relaxed);
    tracing::info!("Copy engine RESUMED via control API");
    (StatusCode::OK, Json(json!({ "status": "running" })))
}

/// GET /api/control/status — Current system status.
pub async fn status(State(state): State<AppState>) -> impl IntoResponse {
    let paused = state.pause_flag.load(Ordering::Relaxed);
    let mode = if state.config.dry_run || state.wallet.is_none() {
        "dry_run"
    } else {
        "live"
    };

    let wallet_address = state.wallet.as_ref().map(|w| w.wallet_address());

    let usdc_balance = if let Some(bc) = &state.balance_checker {
        bc.get_usdc_balance().await.ok().map(|b| b.to_string())
    } else {
        None
    };

    Json(json!({
        "mode": mode,
        "paused": paused,
        "wallet": wallet_address,
        "usdc_balance": usdc_balance,
        "copy_enabled": state.config.copy_enabled,
    }))
}

/// POST /api/control/cancel-all — Cancel all open orders on the CLOB.
pub async fn cancel_all(State(state): State<AppState>) -> impl IntoResponse {
    let Some(tc) = &state.trading_client else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "no trading client available (monitor-only mode)" })),
        );
    };

    match tc.cancel_all_orders().await {
        Ok(()) => {
            tracing::warn!("All open orders cancelled via control API");
            (StatusCode::OK, Json(json!({ "status": "all_cancelled" })))
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to cancel all orders");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
        }
    }
}
