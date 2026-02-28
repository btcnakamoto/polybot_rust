use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::db::order_repo;
use crate::models::CopyOrder;
use crate::AppState;

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

pub async fn list(State(state): State<AppState>) -> Json<ApiResponse<Vec<CopyOrder>>> {
    match order_repo::get_pending_orders(&state.db).await {
        Ok(orders) => Json(ApiResponse {
            success: true,
            data: Some(orders),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}
