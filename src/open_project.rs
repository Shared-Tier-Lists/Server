use crate::util::get_string_field;
use crate::AppState;
use axum::extract::State;
use axum::Json;
use http::StatusCode;
use mongodb::bson::{doc, oid::ObjectId, Document};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct OpenProjectRequest {
    id: ObjectId,
}

#[derive(Serialize)]
pub struct OpenProjectResponse {
    tier_rows_html: String,
    image_carousel_html: String,
}

pub async fn open_project(
    State(app_state): State<Arc<AppState>>,
    Json(payload): Json<OpenProjectRequest>,
) -> Result<Json<Option<OpenProjectResponse>>, StatusCode> {
    let tier_lists = app_state.db.collection::<Document>("tier_lists");

    let tier_list_opt = tier_lists
        .find_one(doc! { "_id": payload.id })
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    if let Some(tier_list) = tier_list_opt {
        let res = OpenProjectResponse {
            tier_rows_html: get_string_field(&tier_list, "tier_rows_html")
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
            image_carousel_html: get_string_field(&tier_list, "image_carousel_html")
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        };
        Ok(Json(Some(res)))
    } else {
        Ok(Json(None))
    }
}
