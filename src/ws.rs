use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::extract::ws::{Message, WebSocket};
use axum_core::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use http::StatusCode;
use mongodb::bson::{doc, Document, Uuid};
use mongodb::bson::oid::{Error, ObjectId};
use mongodb::Collection;
use rand::random;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;
use crate::AppState;
use crate::types::ProjectContents;


#[derive(Debug, Deserialize)]
pub struct ProjectInfo {
    project_id: String,
    user_id: String
}

async fn handle_socket(
    mut socket: WebSocket,
    state: Arc<AppState>,
    tx: Sender<ProjectContents>,
    project_id: ObjectId
) {
    tracing::debug!("Upgraded");
    let (mut sender, mut receiver) = socket.split();

    let mut rx = tx.subscribe();

    let mut send_task = tokio::spawn(async move {
        while let Ok(project_contents) = rx.recv().await {
            match serde_json::to_string(&project_contents) {
                Ok(json) => {
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
                Err(err) => {
                    tracing::debug!("Failed to serialize project_contents: {}", err);
                    break;
                }
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            match serde_json::from_str::<ProjectContents>(text.as_str()) {
                Ok(project_contents) => {
                    let _ = tx.send(project_contents);
                }
                Err(_) => {
                    break;
                }
            }

        }
    });

    tokio::select! {
        _ = &mut send_task => { recv_task.abort(); }
        _ = &mut recv_task => { send_task.abort(); }
    }

    state.live_sessions.lock().await.remove(&project_id);
}

async fn shared_session_broadcast_sender(
    state: Arc<AppState>,
    project_id: ObjectId
) -> Result<Sender<ProjectContents>, StatusCode> {
    tracing::debug!("Getting session");
    let tier_lists = state.db.collection::<Document>("tier_lists");

    let tier_list_opt = tier_lists.find_one(doc! { "_id": project_id }).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match tier_list_opt {
        None => {
            Err(StatusCode::NOT_FOUND)
        }
        Some(tier_list) => {
            let tier_list_id = tier_list.get_object_id("_id")
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            let mut guard = state.live_sessions.lock().await;

            match guard.get(&tier_list_id) {
                Some(tx) => {
                    tracing::debug!("Session already started");
                    Ok(tx.clone())
                }
                None => {
                    tracing::debug!("Session not yet started");
                    const BROADCAST_CHANNEL_CAPACITY: usize = 64;
                    let (tx, _rx) = broadcast::channel(BROADCAST_CHANNEL_CAPACITY);
                    guard.insert(project_id, tx.clone());
                    tracing::debug!("Session created");
                    Ok(tx)
                }
            }
        }
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(project_info_path): Query<ProjectInfo>,
    State(state): State<Arc<AppState>>
) -> Result<impl IntoResponse, StatusCode> {
    let project_id = ObjectId::from_str(&project_info_path.project_id)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // todo: verify user can join session
    // let user_id = ObjectId::from_str(&project_info_path.user_id)
    //     .map_err(StatusCode::BAD_REQUEST)?;

    let tx = shared_session_broadcast_sender(state.clone(), project_id).await?;
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state.clone(), tx, project_id)))
}
