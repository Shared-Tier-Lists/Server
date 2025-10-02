use std::sync::Arc;
use axum::extract::State;
use axum::Json;
use http::StatusCode;
use mongodb::bson::{doc, Document, oid::ObjectId};
use mongodb::Database;
use serde::{Deserialize, Serialize};

use crate::util::get_string_field;

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
    State(db): State<Arc<Database>>,
    Json(payload): Json<OpenProjectRequest>,
) -> Result<Json<Option<OpenProjectResponse>>, StatusCode> {
    let tier_lists = db.collection::<Document>("tier_lists");

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
