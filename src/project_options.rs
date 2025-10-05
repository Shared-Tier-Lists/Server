use crate::db_constants::{Collections, ProjectFields};
use crate::{AppState, ProjectContents};
use axum::extract::State;
use axum::Json;
use http::StatusCode;
use mongodb::bson::{doc, oid::ObjectId, Document};
use serde::Deserialize;
use std::sync::Arc;
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use crate::authentication::authenticate_user;
use crate::invite::invite_users;

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    user_id: ObjectId,
    project_name: String,
    template_link: String,
    tier_container_html: String,
    image_carousel_html: String,
    initial_invitations: Vec<String>,
}

#[derive(Deserialize)]
pub struct OpenProjectRequest {
    user_id: ObjectId,
    project_id: ObjectId,
}

pub async fn create_project(
    State(app_state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(payload): Json<CreateProjectRequest>,
) -> Result<StatusCode, StatusCode> {
    authenticate_user(payload.user_id, app_state.clone(), auth).await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let projects = app_state.db.collection::<Document>(Collections::PROJECTS);
    
    let project = doc! {
        ProjectFields::NAME: payload.project_name,
        ProjectFields::TEMPLATE_LINK: payload.template_link,
        ProjectFields::CONTRIBUTORS: [],
        ProjectFields::TIER_CONTAINER_HTML: payload.tier_container_html.clone(),
        ProjectFields::IMAGE_CAROUSEL_HTML: payload.image_carousel_html.clone()
    };

    projects.insert_one(project.clone()).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    invite_users(app_state.db.clone(), project, payload.initial_invitations).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(StatusCode::CREATED)
}

pub async fn open_project(
    State(app_state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(payload): Json<OpenProjectRequest>,
) -> Result<Json<ProjectContents>, StatusCode> {
    authenticate_user(payload.user_id, app_state.clone(), auth).await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let projects = app_state.db.collection::<Document>(Collections::PROJECTS);

    let project_opt = projects
        .find_one(doc! { ProjectFields::ID: payload.project_id })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(project) = project_opt {
        let res = ProjectContents {
            tier_container_html: project.get_str(ProjectFields::TIER_CONTAINER_HTML)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.to_string(),
            image_carousel_html: project.get_str(ProjectFields::IMAGE_CAROUSEL_HTML)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.to_string(),
        };
        Ok(Json(res))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
