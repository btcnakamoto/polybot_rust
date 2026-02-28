use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::response::IntoResponse;

use crate::AppState;

pub async fn render(State(state): State<AppState>) -> impl IntoResponse {
    let body = state.metrics_handle.render();
    ([(CONTENT_TYPE, "text/plain; version=0.0.4")], body)
}
