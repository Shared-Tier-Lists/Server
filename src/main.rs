mod error;
mod open_project;
mod get_user_projects;
mod ws;
mod types;

use std::collections::{HashMap, HashSet};
use crate::get_user_projects::get_user_projects;
use axum::response::IntoResponse;
use axum::{
    routing::get
    , Router,
};
use dotenv::dotenv;
use mongodb::{options::ClientOptions, Client};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use axum::routing::{any, post};
use futures_util::SinkExt;
use mongodb::bson::oid::ObjectId;
use tokio::sync::{broadcast, Mutex};
use crate::open_project::open_project;

use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use crate::types::ProjectContents;
use crate::ws::ws_handler;

struct AppState {
    db: mongodb::Database,
    live_sessions: Mutex<HashMap<ObjectId, broadcast::Sender<ProjectContents>>>,
}

#[tokio::main]
async fn main() -> mongodb::error::Result<()> {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=trace", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let uri = env::var("MONGODB_URI").expect("Error: No MONGODB_URI");
    let client_options = ClientOptions::parse(uri).await?;
    let client = Client::with_options(client_options)?;
    let db = client.database("shared_tier_lists");

    let app_state = AppState {
        db,
        live_sessions: Mutex::new(HashMap::new()),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/open-project-list", post(get_user_projects))
        .route("/open-project", post(open_project))
        .route("/ws", any(ws_handler))
        .layer(cors)
        .with_state(Arc::new(app_state));
    

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    tracing::debug!("listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}
