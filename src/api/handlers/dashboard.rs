use axum::extract::State;
use axum::Json;
use rust_decimal::Decimal;
use serde::Serialize;

use crate::db::{position_repo, whale_repo};
use crate::AppState;

#[derive(Serialize)]
pub struct DashboardSummary {
    pub tracked_whales: i64,
    pub active_positions: i64,
    pub total_pnl: String,
    pub today_pnl: String,
    pub open_positions: i64,
}

pub async fn summary(State(state): State<AppState>) -> Json<DashboardSummary> {
    let whales = whale_repo::get_active_whales(&state.db)
        .await
        .unwrap_or_default();
    let tracked_whales = whales.len() as i64;

    let open_positions = position_repo::count_open_positions(&state.db)
        .await
        .unwrap_or(0);

    let today_pnl = position_repo::get_daily_realized_pnl(&state.db)
        .await
        .unwrap_or(Decimal::ZERO);

    let total_pnl: Decimal = whales
        .iter()
        .filter_map(|w| w.total_pnl)
        .sum();

    Json(DashboardSummary {
        tracked_whales,
        active_positions: open_positions,
        total_pnl: total_pnl.to_string(),
        today_pnl: today_pnl.to_string(),
        open_positions,
    })
}
