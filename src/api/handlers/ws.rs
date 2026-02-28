use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;

use crate::AppState;

pub async fn handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    tracing::info!("Dashboard WebSocket client connected");

    let mut rx = state.ws_tx.subscribe();

    loop {
        tokio::select! {
            // Forward broadcast messages to client
            msg = rx.recv() => {
                match msg {
                    Ok(ws_msg) => {
                        match serde_json::to_string(&ws_msg) {
                            Ok(json) => {
                                if socket.send(Message::Text(json)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::error!(error = %e, "Failed to serialize WsMessage");
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "Dashboard WS client lagged");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
            // Handle incoming messages from client (ping/pong, close)
            client_msg = socket.recv() => {
                match client_msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(_)) => {} // ignore text/binary from client
                    Some(Err(_)) => break,
                }
            }
        }
    }

    tracing::info!("Dashboard WebSocket client disconnected");
}
