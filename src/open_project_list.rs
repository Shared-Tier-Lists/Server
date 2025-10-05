use crate::db_constants::{Collections, ProjectFields, UserFields};
use crate::error::SharedTierListError::StatusCodeError;
use crate::{error, AppState};
use axum::extract::State;
use axum::Json;
use http::StatusCode;
use mongodb::bson::oid::ObjectId;
use mongodb::bson::{doc, Document};
use mongodb::Collection;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use axum_extra::TypedHeader;
use headers::Authorization;
use headers::authorization::Bearer;
use crate::authentication::authenticate_user;

#[derive(Deserialize, Debug)]
pub struct GetProjectsRequest {
    user_id: ObjectId,
    template_link: String
}

#[derive(Serialize, Debug)]
pub struct GetProjectsResponse {
    projects: Vec<Project>
}

#[derive(Serialize, Debug)]
struct Project {
    project_id: ObjectId,
    name: String,
    template_link: String,
}


async fn query_project(
    id: ObjectId,
    projects: &Collection<Document>
) -> error::Result<Option<Project>> {
    let project_opt = projects.find_one(doc! { ProjectFields::ID: id }).await?;

    if let Some(project) = project_opt {
        Ok(Some(Project {
            project_id: id,
            name: project.get_str(ProjectFields::NAME)?.to_string(),
            template_link: project.get_str(ProjectFields::TEMPLATE_LINK)?.to_string()
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
    let projects = app_state.db.collection::<Document>(Collections::PROJECTS);

    let user_project_ids = user.get_array(UserFields::PROJECTS)?;
    let mut user_tier_lists = vec![];

    for id in user_project_ids {
        let tier_list_id = match id.as_object_id() {
            Some(id) => id,
            None => return Err(StatusCodeError(StatusCode::INTERNAL_SERVER_ERROR))
        };

        let tier_list_opt = query_project(tier_list_id, &projects).await?;

        if let Some(tier_list) = tier_list_opt {
            if &tier_list.template_link == template_link {
                user_tier_lists.push(tier_list);
            }
        }
    }

    Ok(GetProjectsResponse {
        projects: user_tier_lists
    })
}

pub async fn open_project_list(
    State(app_state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(payload): Json<GetProjectsRequest>,
) -> Result<Json<GetProjectsResponse>, StatusCode> {
    let user = authenticate_user(app_state.clone(), auth).await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    
    let projects = query_user_projects(app_state, &user, &payload.template_link).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(projects))
}
