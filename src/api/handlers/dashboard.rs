use axum::extract::State;
use axum::Json;
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::db::{basket_repo, position_repo, whale_repo};
use crate::AppState;

#[derive(Serialize)]
pub struct DashboardSummary {
    pub tracked_whales: i64,
    pub active_positions: i64,
    pub total_pnl: String,
    pub today_pnl: String,
    pub open_positions: i64,
    pub active_baskets: i64,
    pub recent_consensus_count: i64,
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

    let active_baskets = basket_repo::count_active_baskets(&state.db)
        .await
        .unwrap_or(0);

    let since_24h = Utc::now() - Duration::hours(24);
    let recent_consensus_count = basket_repo::count_recent_consensus_signals(&state.db, since_24h)
        .await
        .unwrap_or(0);

    Json(DashboardSummary {
        tracked_whales,
        active_positions: open_positions,
        total_pnl: total_pnl.to_string(),
        today_pnl: today_pnl.to_string(),
        open_positions,
        active_baskets,
        recent_consensus_count,
    })
}
