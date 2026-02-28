use axum::extract::State;
use axum::Json;
use rust_decimal::Decimal;
use serde::Serialize;

use crate::AppState;

#[derive(Serialize)]
pub struct PnlDataPoint {
    pub date: String,
    pub daily_pnl: String,
    pub cumulative_pnl: String,
}

#[derive(Serialize)]
pub struct PerformanceMetrics {
    pub total_trades: i64,
    pub win_count: i64,
    pub loss_count: i64,
    pub win_rate: String,
    pub total_profit: String,
    pub avg_profit_per_trade: String,
    pub best_trade: String,
    pub worst_trade: String,
}

pub async fn pnl_history(State(state): State<AppState>) -> Json<Vec<PnlDataPoint>> {
    let rows: Vec<(chrono::NaiveDate, Option<Decimal>)> = sqlx::query_as(
        r#"
        SELECT closed_at::date AS day, SUM(realized_pnl) AS daily_pnl
        FROM positions
        WHERE status = 'closed' AND realized_pnl IS NOT NULL AND closed_at IS NOT NULL
        GROUP BY closed_at::date
        ORDER BY day
        "#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut cumulative = Decimal::ZERO;
    let points: Vec<PnlDataPoint> = rows
        .into_iter()
        .map(|(day, daily)| {
            let daily_pnl = daily.unwrap_or(Decimal::ZERO);
            cumulative += daily_pnl;
            PnlDataPoint {
                date: day.to_string(),
                daily_pnl: daily_pnl.to_string(),
                cumulative_pnl: cumulative.to_string(),
            }
        })
        .collect();

    Json(points)
}

pub async fn performance(State(state): State<AppState>) -> Json<PerformanceMetrics> {
    let total_trades: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM positions WHERE status = 'closed' AND realized_pnl IS NOT NULL",
    )
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    let win_count: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM positions WHERE status = 'closed' AND realized_pnl > 0",
    )
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    let loss_count: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM positions WHERE status = 'closed' AND realized_pnl IS NOT NULL AND realized_pnl <= 0",
    )
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    let total_profit: Decimal = sqlx::query_as::<_, (Option<Decimal>,)>(
        "SELECT COALESCE(SUM(realized_pnl), 0) FROM positions WHERE status = 'closed' AND realized_pnl IS NOT NULL",
    )
    .fetch_one(&state.db)
    .await
    .map(|r| r.0.unwrap_or(Decimal::ZERO))
    .unwrap_or(Decimal::ZERO);

    let best_trade: Decimal = sqlx::query_as::<_, (Option<Decimal>,)>(
        "SELECT MAX(realized_pnl) FROM positions WHERE status = 'closed' AND realized_pnl IS NOT NULL",
    )
    .fetch_one(&state.db)
    .await
    .map(|r| r.0.unwrap_or(Decimal::ZERO))
    .unwrap_or(Decimal::ZERO);

    let worst_trade: Decimal = sqlx::query_as::<_, (Option<Decimal>,)>(
        "SELECT MIN(realized_pnl) FROM positions WHERE status = 'closed' AND realized_pnl IS NOT NULL",
    )
    .fetch_one(&state.db)
    .await
    .map(|r| r.0.unwrap_or(Decimal::ZERO))
    .unwrap_or(Decimal::ZERO);

    let win_rate = if total_trades > 0 {
        Decimal::from(win_count) / Decimal::from(total_trades)
    } else {
        Decimal::ZERO
    };

    let avg_profit = if total_trades > 0 {
        total_profit / Decimal::from(total_trades)
    } else {
        Decimal::ZERO
    };

    Json(PerformanceMetrics {
        total_trades,
        win_count,
        loss_count,
        win_rate: win_rate.to_string(),
        total_profit: total_profit.to_string(),
        avg_profit_per_trade: avg_profit.to_string(),
        best_trade: best_trade.to_string(),
        worst_trade: worst_trade.to_string(),
    })
}
