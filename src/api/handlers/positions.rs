use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::db::position_repo;
use crate::models::Position;
use crate::AppState;

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

pub async fn list(State(state): State<AppState>) -> Json<ApiResponse<Vec<Position>>> {
    match position_repo::get_all_positions(&state.db).await {
        Ok(positions) => Json(ApiResponse {
            success: true,
            data: Some(positions),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}
