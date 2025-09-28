use axum::extract::State;
use axum::{
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use dotenv::dotenv;
use mongodb::{bson::doc, options::ClientOptions, Client, Database};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;


#[derive(Deserialize, Debug)]
struct UserLogin {
    username: String,
    password: String,
}

#[derive(Serialize, Debug)]
struct OutData {
    res: String,
}


async fn submit_handler(State(db): State<Arc<Database>>, Json(payload): Json<UserLogin>) -> (StatusCode, Json<OutData>) {
    let collection = db.collection::<mongodb::bson::Document>("users");

    StatusCode::BAD_REQUEST
}


#[tokio::main]
async fn main() -> mongodb::error::Result<()> {
    dotenv().ok();

    let uri = env::var("MONGODB_URI").expect("Error: No MONGODB_URI");
    let client_options = ClientOptions::parse(uri).await?;
    let client = Client::with_options(client_options)?;

    let db = client.database("shared_tier_lists");


    let app = Router::new()
        .route("/submit", post(submit_handler))
        .with_state(Arc::new(db));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
