use axum::extract::{Path, State};
use axum::Json;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::db::{market_repo, order_repo, position_repo};
use crate::models::Position;
use crate::AppState;

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct PositionEnriched {
    #[serde(flatten)]
    pub position: Position,
    pub market_slug: Option<String>,
    pub market_question: Option<String>,
}

pub async fn list(State(state): State<AppState>) -> Json<ApiResponse<Vec<PositionEnriched>>> {
    match position_repo::get_all_positions(&state.db).await {
        Ok(positions) => {
            let mut enriched = Vec::with_capacity(positions.len());
            for pos in positions {
                let (market_slug, market_question) =
                    match market_repo::get_market_info(&state.db, &pos.market_id).await {
                        Ok(Some((slug, question))) => (slug, question),
                        _ => (None, None),
                    };
                enriched.push(PositionEnriched {
                    position: pos,
                    market_slug,
                    market_question,
                });
            }
            Json(ApiResponse {
                success: true,
                data: Some(enriched),
                error: None,
            })
        }
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}

#[derive(Deserialize)]
pub struct CloseRequest {
    pub price: Option<String>,
}

pub async fn close(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<CloseRequest>,
) -> Json<ApiResponse<Position>> {
    // 1. Fetch position
    let pos = match position_repo::get_position_by_id(&state.db, id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return Json(ApiResponse {
                success: false,
                data: None,
                error: Some("Position not found".into()),
            });
        }
        Err(e) => {
            return Json(ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            });
        }
    };

    let status = pos.status.as_deref().unwrap_or("open");
    if status != "open" {
        return Json(ApiResponse {
            success: false,
            data: None,
            error: Some(format!("Position status is '{}', expected 'open'", status)),
        });
    }

    // 2. Determine exit price
    let exit_price = if let Some(ref price_str) = body.price {
        match Decimal::from_str(price_str) {
            Ok(p) => p,
            Err(_) => {
                return Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some("Invalid price format".into()),
                });
            }
        }
    } else {
        // Auto-fetch best bid from orderbook
        let Some(ref clob) = state.clob_client else {
            return Json(ApiResponse {
                success: false,
                data: None,
                error: Some("No CLOB client configured — provide price manually".into()),
            });
        };
        match clob.get_order_book(&pos.token_id).await {
            Ok(book) => {
                if let Some(best_bid) = book.bids.iter().map(|b| b.price).max() {
                    best_bid
                } else {
                    return Json(ApiResponse {
                        success: false,
                        data: None,
                        error: Some("No bids in orderbook".into()),
                    });
                }
            }
            Err(e) => {
                return Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Failed to fetch orderbook: {}", e)),
                });
            }
        }
    };

    let dry_run = state.config.dry_run || state.trading_client.is_none();

    if !dry_run {
        // --- Live mode ---
        let tc = state.trading_client.as_ref().unwrap();
        match tc
            .place_limit_order(&pos.token_id, "SELL", pos.size, exit_price)
            .await
        {
            Ok(resp) => {
                if resp.success {
                    // Record exit order
                    if let Ok(exit_order) = order_repo::insert_order(
                        &state.db,
                        uuid::Uuid::nil(),
                        &pos.market_id,
                        &pos.token_id,
                        "SELL",
                        pos.size,
                        exit_price,
                        "exit",
                    )
                    .await
                    {
                        let clob_id = if resp.order_id.is_empty() {
                            ""
                        } else {
                            &resp.order_id
                        };
                        let _ = order_repo::mark_order_submitted(&state.db, exit_order.id, clob_id)
                            .await;
                    }

                    // Mark position as exiting
                    if let Err(e) =
                        position_repo::mark_position_exiting(&state.db, pos.id, "manual").await
                    {
                        tracing::error!(error = %e, "Failed to mark position as exiting");
                    }
                } else {
                    let msg = resp.error_msg.unwrap_or_default();
                    return Json(ApiResponse {
                        success: false,
                        data: None,
                        error: Some(format!("Order rejected: {}", msg)),
                    });
                }
            }
            Err(e) => {
                return Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some(format!("Failed to place exit order: {}", e)),
                });
            }
        }
    } else {
        // --- Dry-run mode: close immediately ---
        let realized_pnl = (exit_price - pos.avg_entry_price) * pos.size;
        if let Err(e) =
            position_repo::close_position_with_reason(&state.db, pos.id, realized_pnl, "manual")
                .await
        {
            return Json(ApiResponse {
                success: false,
                data: None,
                error: Some(format!("Failed to close position: {}", e)),
            });
        }
    }

    // Return updated position
    match position_repo::get_position_by_id(&state.db, id).await {
        Ok(Some(updated)) => Json(ApiResponse {
            success: true,
            data: Some(updated),
            error: None,
        }),
        Ok(None) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some("Position disappeared after update".into()),
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}
