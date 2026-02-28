use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;
use uuid::Uuid;

use crate::db::{trade_repo, whale_repo};
use crate::models::{Whale, WhaleTrade};
use crate::AppState;

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

pub async fn list(State(state): State<AppState>) -> Json<ApiResponse<Vec<Whale>>> {
    match whale_repo::get_active_whales(&state.db).await {
        Ok(whales) => Json(ApiResponse {
            success: true,
            data: Some(whales),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}

pub async fn detail(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Result<Json<ApiResponse<Whale>>, StatusCode> {
    match whale_repo::get_whale_by_address(&state.db, &address).await {
        Ok(Some(whale)) => Ok(Json(ApiResponse {
            success: true,
            data: Some(whale),
            error: None,
        })),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn trades(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Json<ApiResponse<Vec<WhaleTrade>>> {
    match trade_repo::get_trades_by_whale(&state.db, id).await {
        Ok(trades) => Json(ApiResponse {
            success: true,
            data: Some(trades),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}
