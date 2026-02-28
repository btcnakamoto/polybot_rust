use axum::extract::{Path, State};
use axum::Json;
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

use crate::db::basket_repo;
use crate::errors::AppError;
use crate::intelligence::basket::check_admission;
use crate::models::{ConsensusSignal, Whale, WhaleBasket};
use crate::AppState;

use super::whales::ApiResponse;

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct CreateBasketRequest {
    pub name: String,
    pub category: String,
    pub consensus_threshold: Option<Decimal>,
    pub time_window_hours: Option<i32>,
}

#[derive(Deserialize)]
pub struct AddWhaleRequest {
    pub whale_id: Uuid,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/baskets — list active baskets
pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<WhaleBasket>>>, AppError> {
    let baskets = basket_repo::get_active_baskets(&state.db).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: Some(baskets),
        error: None,
    }))
}

/// GET /api/baskets/{id} — basket detail
pub async fn detail(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<WhaleBasket>>, AppError> {
    let basket = basket_repo::get_basket_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("basket not found".into()))?;

    Ok(Json(ApiResponse {
        success: true,
        data: Some(basket),
        error: None,
    }))
}

/// GET /api/baskets/{id}/whales — whales in basket
pub async fn whales(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<Whale>>>, AppError> {
    let whales = basket_repo::get_basket_whales(&state.db, id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: Some(whales),
        error: None,
    }))
}

/// POST /api/baskets — create a new basket
pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateBasketRequest>,
) -> Result<Json<ApiResponse<WhaleBasket>>, AppError> {
    let threshold = body
        .consensus_threshold
        .unwrap_or(state.config.basket_consensus_threshold);
    let window = body
        .time_window_hours
        .unwrap_or(state.config.basket_time_window_hours);

    let basket = basket_repo::create_basket(
        &state.db,
        &body.name,
        &body.category,
        threshold,
        window,
        state.config.basket_min_wallets,
        state.config.basket_max_wallets,
    )
    .await?;

    Ok(Json(ApiResponse {
        success: true,
        data: Some(basket),
        error: None,
    }))
}

/// POST /api/baskets/{id}/whales — add whale to basket (with admission check)
pub async fn add_whale(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<AddWhaleRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    // Check basket exists and get limits
    let basket = basket_repo::get_basket_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("basket not found".into()))?;

    // Check capacity
    let current_count = basket_repo::count_basket_whales(&state.db, id).await?;
    if current_count >= basket.max_wallets as i64 {
        return Err(AppError::BadRequest(format!(
            "basket is full ({}/{})",
            current_count, basket.max_wallets
        )));
    }

    // Get whale for admission check
    let whale = sqlx::query_as::<_, Whale>("SELECT * FROM whales WHERE id = $1")
        .bind(body.whale_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("whale not found".into()))?;

    // Compute months active
    let months_active = whale
        .created_at
        .map(|c| {
            let now = chrono::Utc::now();
            (now - c).num_days() / 30
        })
        .unwrap_or(0);

    let total_trades = whale.total_trades.unwrap_or(0);
    let avg_monthly = if months_active > 0 {
        Decimal::from(total_trades as i64) / Decimal::from(months_active)
    } else {
        Decimal::from(total_trades as i64)
    };

    let admission = check_admission(
        whale.win_rate.unwrap_or(Decimal::ZERO),
        whale.classification.as_deref(),
        months_active,
        total_trades,
        avg_monthly,
    );

    if let crate::intelligence::AdmissionResult::Rejected(reason) = admission {
        return Err(AppError::BadRequest(format!(
            "whale does not qualify: {}",
            reason
        )));
    }

    basket_repo::add_whale_to_basket(&state.db, id, body.whale_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: Some(()),
        error: None,
    }))
}

/// DELETE /api/baskets/{id}/whales/{whale_id} — remove whale from basket
pub async fn remove_whale(
    State(state): State<AppState>,
    Path((id, whale_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    basket_repo::remove_whale_from_basket(&state.db, id, whale_id).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: Some(()),
        error: None,
    }))
}

/// GET /api/baskets/{id}/consensus — consensus history for a basket
pub async fn consensus_history(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<ConsensusSignal>>>, AppError> {
    let signals = basket_repo::get_consensus_signals_for_basket(&state.db, id, 50).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: Some(signals),
        error: None,
    }))
}

/// GET /api/consensus/recent — global recent consensus signals
pub async fn recent_consensus(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<ConsensusSignal>>>, AppError> {
    let signals = basket_repo::get_recent_consensus_signals(&state.db, 50).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: Some(signals),
        error: None,
    }))
}
