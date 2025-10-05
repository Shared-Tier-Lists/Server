use crate::db_constants::{Collections, ProjectFields, UserFields};
use crate::{error, AppState, ProjectContents};
use axum::extract::State;
use axum::Json;
use http::StatusCode;
use mongodb::bson::{doc, oid::ObjectId, Array, Document};
use serde::Deserialize;
use std::sync::Arc;
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use mongodb::Database;
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

#[derive(Deserialize)]
pub struct DeleteProjectRequest {
    user_id: ObjectId,
    project_id: ObjectId,
}

pub async fn create_project(
    State(app_state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(payload): Json<CreateProjectRequest>,
) -> Result<StatusCode, StatusCode> {
    authenticate_user(app_state.clone(), auth).await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let projects = app_state.db.collection::<Document>(Collections::PROJECTS);
    
    let project = doc! {
        ProjectFields::NAME: payload.project_name,
        ProjectFields::TEMPLATE_LINK: payload.template_link,
        ProjectFields::OWNER: payload.user_id,
        ProjectFields::CONTRIBUTORS: [],
        ProjectFields::TIER_CONTAINER_HTML: payload.tier_container_html.clone(),
        ProjectFields::IMAGE_CAROUSEL_HTML: payload.image_carousel_html.clone()
    };

    let project = projects.insert_one(project.clone()).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::debug!("Created Project");

    let project_id = match project.inserted_id.as_object_id() {
        Some(id) => id,
        None => return Err(StatusCode::INTERNAL_SERVER_ERROR)
    };

    let users = app_state.db.collection::<Document>(Collections::USERS);
    users.update_one(
        doc! { UserFields::ID: payload.user_id },
        doc! { "$addToSet": { UserFields::PROJECTS: project_id } })
        .await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    invite_users(app_state.db.clone(), project_id, payload.initial_invitations).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::debug!("Invited Users");
    
    Ok(StatusCode::CREATED)
}

pub async fn open_project(
    State(app_state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(payload): Json<OpenProjectRequest>,
) -> Result<Json<ProjectContents>, StatusCode> {
    authenticate_user(app_state.clone(), auth).await
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

pub async fn remove_project_from_user_list(
    db: Database,
    user_id: ObjectId,
    project_id: ObjectId
) -> error::Result<()> {
    let users = db.collection::<Document>(Collections::USERS);

    users.find_one_and_update(
        doc! { UserFields::ID: user_id },
        doc! { "$pull": { UserFields::PROJECTS: project_id } }
    ).await?;

    Ok(())
}

pub async fn remove_project_from_contributors_lists(
    db: Database,
    project_id: ObjectId,
    contributors: &Array
) -> error::Result<()> {
    for user in contributors {
        if let Some(user_id) = user.as_object_id() {
            remove_project_from_user_list(db.clone(), user_id, project_id).await?;
        }
    }

    Ok(())
}

pub async fn delete_project(
    State(app_state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(payload): Json<DeleteProjectRequest>,
) -> Result<StatusCode, StatusCode> {
    authenticate_user(app_state.clone(), auth).await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let projects = app_state.db.collection::<Document>(Collections::PROJECTS);

    let project_opt = projects.find_one(
        doc! { ProjectFields::ID: payload.project_id }).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match project_opt {
        None => {
            Err(StatusCode::NOT_FOUND)
        }
        Some(project) => {
            let owner = project.get_object_id(ProjectFields::OWNER)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            if owner != payload.user_id {
                return Err(StatusCode::UNAUTHORIZED);
            }

            let project_id = project.get_object_id(ProjectFields::ID)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            remove_project_from_user_list(app_state.db.clone(), owner, project_id).await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            let contributors = project.get_array(ProjectFields::CONTRIBUTORS)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            remove_project_from_contributors_lists(app_state.db.clone(), project_id, contributors).await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            Ok(StatusCode::OK)
        }
    }
}
