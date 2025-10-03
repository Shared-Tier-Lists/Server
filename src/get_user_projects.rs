use crate::{error, AppState};
use axum::extract::State;
use axum::Json;
use http::StatusCode;
use mongodb::bson::oid::ObjectId;
use mongodb::bson::{doc, Document};
use mongodb::Collection;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::error::SharedTierListError::StatusCodeError;

#[derive(Deserialize, Debug)]
pub struct GetProjectsRequest {
    user_id: String,
    template_link: String
}

#[derive(Serialize, Debug)]
pub struct GetProjectsResponse {
    user_id: ObjectId,
    projects: Vec<Project>
}

#[derive(Serialize, Debug)]
struct Project {
    id: ObjectId,
    name: String,
    template_link: String,
}


async fn query_project(
    id: ObjectId,
    tier_lists: &Collection<Document>
) -> error::Result<Option<Project>> {
    let tier_list_opt = tier_lists.find_one(doc! { "_id": id }).await?;

    if let Some(tier_list) = tier_list_opt {
        Ok(Some(Project {
            id,
            name: tier_list.get_str("name")?.to_string(),
            template_link: tier_list.get_str("template_link")?.to_string()
        }))

    } else {
        Ok(None)
    }
}

async fn query_user_projects(
    app_state: Arc<AppState>,
    user: &Document,
    template_link: &String
) -> error::Result<GetProjectsResponse> {
    let tier_lists_collection = app_state.db.collection::<Document>("tier_lists");

    let tier_list_ids = user.get_array("tier_lists")?;
    let mut user_tier_lists = vec![];

    for id in tier_list_ids {
        let tier_list_id = id.as_object_id().expect("id must be ObjectId");
        let tier_list_opt = query_project(tier_list_id, &tier_lists_collection).await?;

        if let Some(tier_list) = tier_list_opt {
            if &tier_list.template_link == template_link {
                user_tier_lists.push(tier_list);
            }
        }
    }

    Ok(GetProjectsResponse {
        user_id: user.get_object_id("_id")?,
        projects: user_tier_lists
    })
}


async fn create_user(
    app_state: Arc<AppState>,
    tier_maker_user_id: &String
) -> error::Result<GetProjectsResponse> {
    let users = app_state.db.collection::<Document>("users");

    let user = doc! {
        "tier_lists": [],
        "user_id": tier_maker_user_id,
    };

    let user_id = match users.insert_one(user).await?.inserted_id.as_object_id() {
        Some(id) => id,
        None => return Err(StatusCodeError(StatusCode::INTERNAL_SERVER_ERROR))
    };

    Ok(GetProjectsResponse {
        user_id,
        projects: vec![]
    })
}

async fn get_user_projects_response(
    app_state: Arc<AppState>,
    user_opt: Option<Document>,
    user_id: &String,
    template_link: &String,
) -> error::Result<GetProjectsResponse> {
    if let Some(user) = user_opt {
        query_user_projects(app_state, &user, template_link).await
    } else {
        create_user(app_state, user_id).await
    }
}


pub async fn get_user_projects(
    State(app_state): State<Arc<AppState>>,
    Json(payload): Json<GetProjectsRequest>,
) -> Result<Json<Option<GetProjectsResponse>>, StatusCode> {
    let users = app_state.db.collection::<Document>("users");

    let user_opt = users
        .find_one(doc! { "user_id": &payload.user_id })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tier_lists = get_user_projects_response(app_state, user_opt, &payload.user_id, &payload.template_link)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(Some(tier_lists)))
}
