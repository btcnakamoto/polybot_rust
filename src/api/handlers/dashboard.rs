use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct DashboardSummary {
    pub tracked_whales: u32,
    pub active_positions: u32,
    pub total_pnl_usdc: f64,
    pub win_rate: f64,
    pub pending_signals: u32,
}

pub async fn summary() -> Json<DashboardSummary> {
    // Phase 0: return mock data
    Json(DashboardSummary {
        tracked_whales: 0,
        active_positions: 0,
        total_pnl_usdc: 0.0,
        win_rate: 0.0,
        pending_signals: 0,
    })
}
