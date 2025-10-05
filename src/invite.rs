use crate::authentication::authenticate_user;
use crate::db_constants::{Collections, ProjectFields, UserFields};
use crate::AppState;
use axum::extract::State;
use axum::Json;
use axum_extra::TypedHeader;
use futures_util::TryStreamExt;
use headers::authorization::Bearer;
use headers::Authorization;
use http::StatusCode;
use mongodb::bson::oid::ObjectId;
use mongodb::bson::{doc, Document};
use mongodb::Database;
use serde::Deserialize;
use std::sync::Arc;


#[derive(Deserialize)]
pub struct InviteRequest {
    user_id: ObjectId,
    project_id: ObjectId,
    emails: Vec<String>
}

pub async fn invite_users(db: Database, project: Document, emails: Vec<String>) -> crate::error::Result<()> {
    let project_id = project.get_object_id(ProjectFields::ID)?;
    let users = db.collection::<Document>(Collections::USERS);
    let projects = db.collection::<Document>(Collections::PROJECTS);

    let filter = doc! { UserFields::EMAIL: { "$in": emails } };
    let mut cursor = users.find(filter).await?;
    let mut invited_user_ids = Vec::new();

    while let Some(user) = cursor.try_next().await? {
        let user_id = user.get_object_id(UserFields::ID)?;

        users.update_one(
            doc! { UserFields::ID: user_id },
            doc! { "$addToSet": { UserFields::PROJECTS: project_id } }
        ).await?;

        invited_user_ids.push(user_id);
    }

    projects.update_one(
        doc! { ProjectFields::ID: project_id },
        doc! { "$addToSet": { ProjectFields::CONTRIBUTORS: { "$each": invited_user_ids } } }
    ).await?;

    Ok(())
}

// todo: add payload to response for failed invitations
pub async fn invite_to_project(
    State(app_state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(payload): Json<InviteRequest>,
) -> Result<StatusCode, StatusCode> {
    authenticate_user(payload.user_id, app_state.clone(), auth).await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let projects = app_state.db.collection::<Document>(Collections::PROJECTS);

    let project_opt = projects
        .find_one(doc! { ProjectFields::ID: payload.project_id })
        .await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(project) = project_opt {
        match invite_users(app_state.db.clone(), project, payload.emails).await {
            Ok(()) => Ok(StatusCode::OK),
            Err(_) => Ok(StatusCode::INTERNAL_SERVER_ERROR)
        }
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
