use std::collections::HashMap;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::db::config_repo;
use crate::AppState;

const ALLOWED_KEYS: &[&str] = &[
    "copy_strategy",
    "bankroll",
    "base_copy_amount",
    "dry_run",
    "copy_enabled",
    "min_signal_win_rate",
    "min_total_trades_for_signal",
    "min_signal_ev",
    "assumed_slippage_pct",
    "signal_notional_liquidity_pct",
    "signal_notional_floor",
    "max_signal_notional",
    "default_stop_loss_pct",
    "default_take_profit_pct",
    "basket_consensus_threshold",
    "basket_time_window_hours",
    "notifications_enabled",
];

#[derive(Serialize)]
pub struct ConfigEntry {
    pub key: String,
    pub value: String,
}

fn defaults_from_config(state: &AppState) -> HashMap<String, String> {
    let c = &state.config;
    let mut m = HashMap::new();
    m.insert("copy_strategy".into(), c.copy_strategy.clone());
    m.insert("bankroll".into(), c.bankroll.to_string());
    m.insert("base_copy_amount".into(), c.base_copy_amount.to_string());
    m.insert("dry_run".into(), c.dry_run.to_string());
    m.insert("copy_enabled".into(), c.copy_enabled.to_string());
    m.insert("min_signal_win_rate".into(), c.min_signal_win_rate.to_string());
    m.insert("min_total_trades_for_signal".into(), c.min_total_trades_for_signal.to_string());
    m.insert("min_signal_ev".into(), c.min_signal_ev.to_string());
    m.insert("assumed_slippage_pct".into(), c.assumed_slippage_pct.to_string());
    m.insert("signal_notional_liquidity_pct".into(), c.signal_notional_liquidity_pct.to_string());
    m.insert("signal_notional_floor".into(), c.signal_notional_floor.to_string());
    m.insert("max_signal_notional".into(), c.max_signal_notional.to_string());
    m.insert("default_stop_loss_pct".into(), c.default_stop_loss_pct.to_string());
    m.insert("default_take_profit_pct".into(), c.default_take_profit_pct.to_string());
    m.insert("basket_consensus_threshold".into(), c.basket_consensus_threshold.to_string());
    m.insert("basket_time_window_hours".into(), c.basket_time_window_hours.to_string());
    m.insert("notifications_enabled".into(), c.notifications_enabled.to_string());
    m
}

pub async fn get_config(State(state): State<AppState>) -> Json<Vec<ConfigEntry>> {
    let mut merged = defaults_from_config(&state);

    if let Ok(db_entries) = config_repo::get_all_config(&state.db).await {
        for entry in db_entries {
            if ALLOWED_KEYS.contains(&entry.key.as_str()) {
                merged.insert(entry.key, entry.value);
            }
        }
    }

    let entries: Vec<ConfigEntry> = merged
        .into_iter()
        .map(|(key, value)| ConfigEntry { key, value })
        .collect();

    Json(entries)
}

#[derive(Deserialize)]
pub struct UpdateConfigRequest {
    pub entries: HashMap<String, String>,
}

pub async fn update_config(
    State(state): State<AppState>,
    Json(body): Json<UpdateConfigRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Filter to only allowed keys
    let filtered: HashMap<String, String> = body
        .entries
        .into_iter()
        .filter(|(k, _)| ALLOWED_KEYS.contains(&k.as_str()))
        .collect();

    if filtered.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "No valid config keys provided"})),
        ));
    }

    match config_repo::upsert_config(&state.db, &filtered).await {
        Ok(()) => Ok(Json(serde_json::json!({
            "success": true,
            "updated": filtered.len()
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}
