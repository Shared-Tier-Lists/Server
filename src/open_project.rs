use crate::types::ProjectContents;
use crate::AppState;
use axum::extract::State;
use axum::Json;
use http::StatusCode;
use mongodb::bson::{doc, oid::ObjectId, Document};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct OpenProjectRequest {
    id: ObjectId,
}

pub async fn open_project(
    State(app_state): State<Arc<AppState>>,
    Json(payload): Json<OpenProjectRequest>,
) -> Result<Json<Option<ProjectContents>>, StatusCode> {
    let tier_lists = app_state.db.collection::<Document>("tier_lists");

    let tier_list_opt = tier_lists
        .find_one(doc! { "_id": payload.id })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(tier_list) = tier_list_opt {
        let res = ProjectContents {
            tier_rows_html: tier_list.get_str("tier_rows_html")
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.to_string(),
            image_carousel_html: tier_list.get_str("image_carousel_html")
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.to_string(),
        };
        Ok(Json(Some(res)))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
