use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::extract::ws::{Message, WebSocket};
use axum_core::response::IntoResponse;
use axum_extra::TypedHeader;
use futures_util::{SinkExt, StreamExt};
use futures_util::stream::{SplitSink, SplitStream};
use headers::Authorization;
use headers::authorization::Bearer;
use http::{HeaderMap, StatusCode};
use mongodb::bson::{doc, Document};
use mongodb::bson::oid::{ObjectId};
use serde::{Deserialize};
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;
use crate::{AppState, ProjectContents};
use crate::authentication::authenticate_user;
use crate::db_constants::{Collections, ProjectFields};


#[derive(Debug, Deserialize)]
pub struct ProjectInfo {
    project_id: String,
    user_id: String
}

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ClientMessage {
    OpenProject {
        jwt: String,
        project_id: String
    },
    EditProject {
        jwt: String,
        project_id: String,
        tier_container_html: String,
        image_carousel_html: String,
    }
}

async fn socket_send_task(mut sender: SplitSink<WebSocket, Message>, mut rx: Receiver<ClientMessage>) {
    while let Ok(msg) = rx.recv().await {
        match msg {
            ClientMessage::OpenProject {
                jwt,
                project_id
            } => {

            }
            ClientMessage::EditProject {
                jwt,
                project_id,
                tier_container_html,
                image_carousel_html
            } => {

            }
        }
        // match serde_json::to_string(&project_contents) {
        //     Ok(json) => {
        //         if sender.send(Message::Text(json.into())).await.is_err() {
        //             break;
        //         }
        //     }
        //     Err(err) => {
        //         tracing::debug!("Failed to serialize project_contents: {}", err);
        //         break;
        //     }
        // }
    }
}

async fn socket_recv_task(mut receiver: SplitStream<WebSocket>, tx: Sender<ProjectContents>) {
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
}

async fn handle_socket(
    socket: WebSocket,
    state: Arc<AppState>,
    tx: Sender<ProjectContents>,
    project_id: ObjectId
) {
    tracing::debug!("Upgraded");
    let (sender, receiver) = socket.split();

    let rx = tx.subscribe();

    let mut send_task = tokio::spawn(async {
        socket_send_task(sender, rx)
    });

    let mut recv_task = tokio::spawn(async {
        socket_recv_task(receiver, tx)
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
    let projects = state.db.collection::<Document>(Collections::PROJECTS);

    let project_opt = projects.find_one(doc! { ProjectFields::ID: project_id }).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match project_opt {
        None => {
            Err(StatusCode::NOT_FOUND)
        }
        Some(_) => {
            let mut live_sessions_guard = state.live_sessions.lock().await;

            match live_sessions_guard.get(&project_id) {
                Some(tx) => {
                    tracing::debug!("Session already started");
                    Ok(tx.clone())
                }
                None => {
                    tracing::debug!("Session not yet started");
                    const BROADCAST_CHANNEL_CAPACITY: usize = 64;
                    let (tx, _rx) = broadcast::channel(BROADCAST_CHANNEL_CAPACITY);
                    live_sessions_guard.insert(project_id, tx.clone());
                    tracing::debug!("Session created");
                    Ok(tx)
                }
            }
        }
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Path(project_info_path): Path<ProjectInfo>,
) -> Result<impl IntoResponse, StatusCode> {
    let project_id = ObjectId::from_str(&project_info_path.project_id)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // todo: verify user can join session
    // let user_id = ObjectId::from_str(&project_info_path.user_id)
    //     .map_err(StatusCode::BAD_REQUEST)?;

    let tx = shared_session_broadcast_sender(state.clone(), project_id).await?;
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state.clone(), tx, project_id)))
}
