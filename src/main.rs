mod error;
mod project_options;
mod open_project_list;
mod ws;
mod db_constants;
mod authentication;
mod invite;

use std::collections::HashMap;
use crate::open_project_list::open_project_list;
use dotenv::dotenv;
use mongodb::{options::ClientOptions, Client};
use std::env;
use std::sync::Arc;
use axum::Router;
use axum::routing::{any, post};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex};
use crate::project_options::{create_project, delete_project, open_project};

use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use crate::authentication::{login, signup};
use crate::invite::invite_to_project;
use crate::ws::ws_handler;


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectContents {
    pub tier_container_html: String,
    pub image_carousel_html: String,
}

struct AppState {
    db: mongodb::Database,
    live_sessions: Mutex<HashMap<ObjectId, broadcast::Sender<ProjectContents>>>,
    jwt_secret_key: String,
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
        jwt_secret_key: env::var("JWT_SECRET_KEY").expect("Error: No JWT_SECRET_KEY"),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/signup", post(signup))
        .route("/login", post(login))
        .route("/open-project-list", post(open_project_list))
        .route("/open-project", post(open_project))
        .route("/create-project", post(create_project))
        .route("/delete_project", post(delete_project))
        .route("/invite-to-project", post(invite_to_project))
        .route("/ws", any(ws_handler))
        .layer(cors)
        .with_state(Arc::new(app_state));
    

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    tracing::debug!("listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}
