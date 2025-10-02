mod error;
mod open_project;
mod get_user_projects;
mod util;

use crate::get_user_projects::get_user_projects;
// use crate::open_project::open_project;
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
use axum::routing::post;
use crate::open_project::open_project;

use tower_http::cors::{Any, CorsLayer};

#[tokio::main]
async fn main() -> mongodb::error::Result<()> {
    dotenv().ok();

    let uri = env::var("MONGODB_URI").expect("Error: No MONGODB_URI");
    let client_options = ClientOptions::parse(uri).await?;
    let client = Client::with_options(client_options)?;
    let db = client.database("shared_tier_lists");

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/tier-lists", post(get_user_projects))
        .route("/open-project", post(open_project))
        .layer(cors)
        .with_state(Arc::new(db));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
