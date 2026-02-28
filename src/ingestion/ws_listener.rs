use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time::{interval, sleep};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::models::{Side, WhaleTradeEvent};
use crate::polymarket::types::{WsSubscribe, WsTrade};

const PING_INTERVAL: Duration = Duration::from_secs(25);
const BASE_RECONNECT_DELAY: Duration = Duration::from_secs(2);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(60);

/// Build a JSON subscribe message for a set of token IDs.
fn build_subscribe_messages(token_ids: &[String]) -> Vec<String> {
    token_ids
        .iter()
        .filter_map(|token_id| {
            let sub = WsSubscribe::market_trades(token_id);
            serde_json::to_string(&sub).ok()
        })
        .collect()
}

/// Run the WebSocket listener loop with dynamic token subscription updates.
///
/// `token_rx` is a `watch::Receiver` that emits updated token ID lists
/// from the market discovery service. When new tokens arrive, the listener
/// sends fresh subscribe messages on the existing connection.
pub async fn run_ws_listener(
    ws_url: String,
    token_rx: watch::Receiver<Vec<String>>,
    tx: mpsc::Sender<WhaleTradeEvent>,
) {
    let mut attempt: u32 = 0;
    let mut token_rx = token_rx;

    loop {
        tracing::info!(url = %ws_url, "Connecting to Polymarket WebSocket...");

        match connect_async(&ws_url).await {
            Ok((ws_stream, _response)) => {
                tracing::info!("WebSocket connected successfully");
                attempt = 0;

                let (mut write, mut read) = ws_stream.split();

                // Subscribe to initial token list
                let current_tokens = token_rx.borrow().clone();
                for msg in build_subscribe_messages(&current_tokens) {
                    if let Err(e) = write.send(Message::Text(msg.into())).await {
                        tracing::error!(error = %e, "Failed to send subscribe message");
                        break;
                    }
                }
                tracing::info!(
                    token_count = current_tokens.len(),
                    "Subscribed to initial token list"
                );

                let mut ping_timer = interval(PING_INTERVAL);
                ping_timer.tick().await; // consume the first immediate tick

                loop {
                    tokio::select! {
                        msg = read.next() => {
                            match msg {
                                Some(Ok(Message::Text(text))) => {
                                    handle_text_message(text.as_ref(), &tx).await;
                                }
                                Some(Ok(Message::Ping(data))) => {
                                    if let Err(e) = write.send(Message::Pong(data)).await {
                                        tracing::warn!(error = %e, "Failed to send pong");
                                        break;
                                    }
                                }
                                Some(Ok(Message::Close(_))) => {
                                    tracing::warn!("WebSocket server sent close frame");
                                    break;
                                }
                                Some(Ok(_)) => {} // Binary, Pong, Frame — ignore
                                Some(Err(e)) => {
                                    tracing::error!(error = %e, "WebSocket read error");
                                    break;
                                }
                                None => {
                                    tracing::warn!("WebSocket stream ended");
                                    break;
                                }
                            }
                        }
                        _ = ping_timer.tick() => {
                            if let Err(e) = write.send(Message::Ping(vec![].into())).await {
                                tracing::warn!(error = %e, "Failed to send ping");
                                break;
                            }
                        }
                        result = token_rx.changed() => {
                            if result.is_err() {
                                tracing::warn!("Token watch channel closed");
                                break;
                            }
                            let new_tokens = token_rx.borrow().clone();
                            tracing::info!(
                                token_count = new_tokens.len(),
                                "Received updated token list — resubscribing"
                            );
                            for msg in build_subscribe_messages(&new_tokens) {
                                if let Err(e) = write.send(Message::Text(msg.into())).await {
                                    tracing::error!(error = %e, "Failed to send subscribe message");
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "WebSocket connection failed");
            }
        }

        // Exponential backoff with cap
        let delay = BASE_RECONNECT_DELAY * 2u32.saturating_pow(attempt);
        let delay = delay.min(MAX_RECONNECT_DELAY);
        attempt = attempt.saturating_add(1);
        tracing::info!(delay_secs = delay.as_secs(), attempt, "Reconnecting...");
        sleep(delay).await;
    }
}

/// Parse an incoming text message, which may be:
/// - A JSON array of trades: `[{...}, {...}]`
/// - A single trade object: `{...}`
/// - A wrapper with a `data` field: `{"data": [{...}]}`
async fn handle_text_message(text: &str, tx: &mpsc::Sender<WhaleTradeEvent>) {
    let trades = parse_trades(text);
    if trades.is_empty() {
        return;
    }

    for ws_trade in trades {
        match convert_ws_trade(&ws_trade) {
            Some(event) => {
                tracing::info!(
                    wallet = %event.wallet,
                    market = %event.market_id,
                    side = %event.side,
                    size = %event.size,
                    price = %event.price,
                    notional = %event.notional,
                    "Trade detected"
                );
                if let Err(e) = tx.send(event).await {
                    tracing::error!(error = %e, "Failed to send WhaleTradeEvent to channel");
                }
            }
            None => {
                tracing::debug!(raw = %text, "Could not convert WS trade to WhaleTradeEvent");
            }
        }
    }
}

fn parse_trades(text: &str) -> Vec<WsTrade> {
    // Try as array of trades
    if let Ok(trades) = serde_json::from_str::<Vec<WsTrade>>(text) {
        return trades;
    }

    // Try as wrapper with `data` field (array)
    if let Ok(wrapper) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(data) = wrapper.get("data") {
            if let Ok(trades) = serde_json::from_value::<Vec<WsTrade>>(data.clone()) {
                return trades;
            }
        }
    }

    // Try as single trade object
    if let Ok(trade) = serde_json::from_str::<WsTrade>(text) {
        return vec![trade];
    }

    // Not a trade message (e.g. subscription ack, heartbeat)
    tracing::trace!(raw = %text, "Non-trade message received");
    Vec::new()
}

fn convert_ws_trade(ws: &WsTrade) -> Option<WhaleTradeEvent> {
    let wallet = ws.taker_address.as_deref().or(ws.maker_address.as_deref())?;
    let market_id = ws.market.as_deref().unwrap_or("unknown");
    let asset_id = ws.asset_id.as_deref().unwrap_or("unknown");
    let side_str = ws.side.as_deref()?;
    let side = Side::from_api_str(side_str)?;

    let size = ws
        .size
        .as_deref()
        .and_then(|s| Decimal::from_str(s).ok())
        .unwrap_or(Decimal::ZERO);

    let price = ws
        .price
        .as_deref()
        .and_then(|s| Decimal::from_str(s).ok())
        .unwrap_or(Decimal::ZERO);

    let notional = size * price;

    let timestamp = ws
        .timestamp
        .as_deref()
        .and_then(|t| {
            // Try parsing as epoch seconds (integer string)
            if let Ok(secs) = t.parse::<i64>() {
                return chrono::DateTime::from_timestamp(secs, 0);
            }
            // Try ISO 8601
            chrono::DateTime::parse_from_rfc3339(t)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        })
        .unwrap_or_else(Utc::now);

    Some(WhaleTradeEvent {
        wallet: wallet.to_string(),
        market_id: market_id.to_string(),
        asset_id: asset_id.to_string(),
        side,
        size,
        price,
        notional,
        timestamp,
    })
}
