use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use axum::extract::{State, WebSocketUpgrade};
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use mongodb::bson::Uuid;
use rand::random;
use crate::AppState;

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl axum_core::response::IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {

    let (mut sender, mut receiver) = socket.split();

    let mut rx = state.tx.subscribe();

    let rand: i64 = random();
    let msg = format!("{rand} joined.");
    tracing::debug!("{msg}");
    let _ = state.tx.send(msg);

    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            // In any websocket error, break loop.
            if sender.send(Message::text(msg)).await.is_err() {
                break;
            }
        }
    });

    let tx = state.tx.clone();

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            let _ = tx.send(format!("{text}"));
        }
    });
}