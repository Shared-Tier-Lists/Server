use std::sync::Arc;
use tokio::sync::broadcast::Receiver;
use axum::extract::{State, WebSocketUpgrade};
use axum::extract::ws::{Message, WebSocket};
use axum_core::response::IntoResponse;
use axum_extra::TypedHeader;
use futures_util::{SinkExt, StreamExt};
use futures_util::stream::{SplitSink, SplitStream};
use headers::Authorization;
use headers::authorization::Bearer;
use http::{StatusCode};
use mongodb::bson::{doc, Document};
use mongodb::bson::oid::{ObjectId};
use tokio::sync::{broadcast, Mutex};
use tokio::sync::broadcast::Sender;
use crate::{error, AppState, ProjectContents};
use crate::authentication::authenticate_user;
use crate::db_constants::{Collections, ProjectFields, UserFields};
use crate::error::SharedTierListError::StatusCodeError;
use crate::ws_types::{ClientMessage, ProjectContentsResponse};


struct WebSocketState {
    user_id: ObjectId,
    project: Arc<Mutex<WebSocketProject>>,
}

struct WebSocketProject {
    project_id: Option<ObjectId>,
    rx: Option<Receiver<ProjectContentsResponse>>,
    tx: Option<Sender<ProjectContentsResponse>>,
}

async fn shared_session_broadcast_sender(
    app_state: Arc<AppState>,
    project_id: ObjectId
) -> error::Result<Sender<ProjectContentsResponse>> {
    tracing::debug!("Getting session");

    let mut live_sessions_guard = app_state.live_sessions.lock().await;

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

async fn check_project_permissions(
    app_state: Arc<AppState>,
    socket_state: Arc<WebSocketState>,
    project_id: ObjectId,
) -> error::Result<()> {
    let user_opt  = app_state.db
        .collection::<Document>(Collections::USERS)
        .find_one(doc! {
            UserFields::ID: socket_state.user_id,
            UserFields::PROJECTS: project_id,
        }).await?;

    match user_opt {
        None => Err(StatusCodeError(StatusCode::INTERNAL_SERVER_ERROR)),
        Some(_) => Ok(())
    }
}

async fn open_project(
    app_state: Arc<AppState>,
    socket_state: Arc<WebSocketState>,
    project_id: ObjectId,
) -> error::Result<()> {
    let projects = app_state.db.collection::<Document>(Collections::PROJECTS);
    let project_opt = projects.find_one(doc! { ProjectFields::ID: project_id }).await?;

    match project_opt {
        None => Err(StatusCodeError(StatusCode::INTERNAL_SERVER_ERROR)),
        Some(project) => {
            if let Ok(tx) = shared_session_broadcast_sender(app_state.clone(), project_id).await {
                let rx = tx.subscribe();

                let mut project_guard = socket_state.project.lock().await;
                project_guard.project_id = Some(project_id);
                project_guard.tx = Some(tx.clone());
                project_guard.rx = Some(rx);
                drop(project_guard);

                let _ = tx.send(ProjectContentsResponse {
                    tier_container_html: project.get_str(ProjectFields::TIER_CONTAINER_HTML)?.to_string(),
                    image_carousel_html: project.get_str(ProjectFields::IMAGE_CAROUSEL_HTML)?.to_string(),
                });
            }

            Ok(())
        }
    }
}

async fn edit_project(
    app_state: Arc<AppState>,
    socket_state: Arc<WebSocketState>,
    project_id: ObjectId,
    project_contents: ProjectContentsResponse,
) -> error::Result<()> {
    let projects = app_state.db.collection::<Document>(Collections::PROJECTS);
    projects.update_one(
        doc! { ProjectFields::ID: project_id },
        doc! {
            ProjectFields::TIER_CONTAINER_HTML: project_contents.tier_container_html.clone(),
            ProjectFields::IMAGE_CAROUSEL_HTML: project_contents.image_carousel_html.clone()
        }
    ).await?;

    match socket_state.project.lock().await.tx.clone() {
        None => Err(StatusCodeError(StatusCode::INTERNAL_SERVER_ERROR))
        Some(tx) => {
            let _ = tx.send(project_contents);
            Ok(())
        }
    }
}

async fn socket_recv_task(
    app_state: Arc<AppState>,
    socket_state: Arc<WebSocketState>,
    mut receiver: SplitStream<WebSocket>
) {
    while let Some(Ok(Message::Text(text))) = receiver.next().await {
        if let Ok(msg) = serde_json::from_str::<ClientMessage>(text.as_str()) {
            match msg {
                ClientMessage::OpenProject {
                    project_id
                } => {
                    if let Err(e) = check_project_permissions(app_state.clone(), socket_state.clone(), project_id).await {
                        tracing::debug!("{e}");
                    }

                    if let Err(e) = open_project(app_state.clone(), socket_state.clone(), project_id).await {
                        tracing::debug!("{e}");
                    }
                }
                ClientMessage::EditProject {
                    tier_container_html,
                    image_carousel_html
                } => {
                    let project_guard = socket_state.project.lock().await;

                    if let Some(project_id) = project_guard.project_id {
                        drop(project_guard);

                        if let Err(e) = check_project_permissions(app_state.clone(), socket_state.clone(), project_id).await {
                            tracing::debug!("{e}");
                        }

                        if let Err(e) = edit_project(
                            app_state.clone(),
                            socket_state.clone(),
                            project_id,
                            ProjectContentsResponse {
                                tier_container_html,
                                image_carousel_html
                            }
                        ).await {
                            tracing::debug!("{e}");
                        }
                    }
                }
            }
        }
    }
}

async fn socket_send_task(
    app_state: Arc<AppState>,
    socket_state: Arc<WebSocketState>,
    mut sender: SplitSink<WebSocket, Message>
) {
    // let project_guard = socket_state.project.lock().await;
    // let rx = project_guard.rx

    while let Ok(msg) = socket_state.rx.recv().await {
        match msg {
            ClientMessage::OpenProject {
                project_id
            } => {
                shared_session_broadcast_sender()
                sender.send()
            }
            ClientMessage::EditProject {
                tier_container_html,
                image_carousel_html
            } => {

            }
        }
    }
}

async fn handle_socket(
    user_id: ObjectId,
    socket: WebSocket,
    app_state: Arc<AppState>,
) {
    tracing::debug!("Upgraded");

    let (sender, receiver) = socket.split();

    let socket_state = Arc::new(WebSocketState {
        user_id,
        project: Arc::new(Mutex::new(WebSocketProject {
            project_id: None,
            rx: None,
            tx: None,
        })),
    });

    let app_state_clone = app_state.clone();
    let socket_state_clone = socket_state.clone();

    let mut recv_task = tokio::spawn(async move {
        socket_recv_task(app_state_clone, socket_state_clone, receiver)
    });

    let mut send_task = tokio::spawn(async move {
        socket_send_task(app_state.clone(), socket_state.clone(), sender)
    });

    tokio::select! {
        _ = &mut send_task => { recv_task.abort(); }
        _ = &mut recv_task => { send_task.abort(); }
    }

}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(app_state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>
) -> Result<impl IntoResponse, StatusCode> {
    let user = authenticate_user(app_state.clone(), auth).await?;
    let user_id = user.get_object_id(UserFields::ID).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(ws.on_upgrade(move |socket| handle_socket(user_id, socket, app_state)))
}
