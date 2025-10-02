use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use axum::extract::{State, WebSocketUpgrade};
use axum::extract::ws::WebSocket;
use crate::AppState;

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl axum_core::response::IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, app_state: Arc<AppState>) {
    while let Some(msg) = socket.recv().await {
        let msg = if let Ok(msg) = msg {
            msg
        } else {
            // Client disconnected
            return;
        };

        // Process the message (e.g., echo it back, broadcast to other clients)
        if socket.send(msg).await.is_err() {
            // Client disconnected
            return;
        }
    }
}