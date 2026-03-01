use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Bearer-token authentication middleware.
///
/// If `API_TOKEN` is set, every request must carry
/// `Authorization: Bearer <token>` matching that value.
/// If `API_TOKEN` is empty / unset, authentication is disabled (dev mode).
pub async fn require_auth(req: Request, next: Next) -> Response {
    let expected = std::env::var("API_TOKEN").unwrap_or_default();

    // No token configured → auth disabled (dev / legacy mode)
    if expected.is_empty() {
        return next.run(req).await;
    }

    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(value) if value.starts_with("Bearer ") => {
            let token = &value[7..];
            if token == expected {
                next.run(req).await
            } else {
                (StatusCode::UNAUTHORIZED, "Invalid token").into_response()
            }
        }
        _ => (StatusCode::UNAUTHORIZED, "Missing or invalid Authorization header").into_response(),
    }
}
