use std::sync::Arc;
use axum::extract::State;
use axum::Json;
use axum_core::response::IntoResponse;
use http::StatusCode;
use mongodb::bson::{doc, Document};
use mongodb::bson::oid::ObjectId;
use mongodb::{Collection, Database};
use serde::{Deserialize, Serialize};
use crate::error;
use crate::util::get_string_field;

#[derive(Deserialize, Debug)]
pub struct GetProjectsRequest {
    user_id: i64,
    template_link: String
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TierList {
    #[serde(rename = "_id")]
    id: ObjectId,
    name: String,
    template_link: String,
    tier_rows_html: String,
    tier_unused_characters_html: String
}


struct User {
    tier_lists: Vec<ObjectId>,
    user_id: i64
}

async fn query_tier_list(
    id: ObjectId,
    tier_lists: &Collection<Document>
) -> error::Result<Option<TierList>> {
    let tier_list_opt = tier_lists.find_one(doc! { "_id": id }).await?;

    if let Some(tier_list) = tier_list_opt {
        Ok(Some(TierList {
            id,
            name: get_string_field(&tier_list, "name")?,
            template_link: get_string_field(&tier_list, "template_link")?,
            tier_rows_html: get_string_field(&tier_list, "tier_rows_html")?,
            tier_unused_characters_html: get_string_field(&tier_list, "tier_unused_characters_html")?,
        }))
    } else {
        Ok(None)
    }
}

async fn query_user_projects(
    db: Arc<Database>,
    user: Document,
    template_link: String
) -> error::Result<Vec<TierList>> {
    let tier_lists_collection = db.collection::<Document>("tier_lists");

    let tier_list_ids = user.get_array("tier_lists")?;
    let mut user_tier_lists = vec![];

    for id in tier_list_ids {
        let tier_list_id = id.as_object_id().expect("id must be ObjectId");
        let tier_list_opt = query_tier_list(tier_list_id, &tier_lists_collection).await?;

        if let Some(tier_list) = tier_list_opt {
            if tier_list.template_link == template_link {
                user_tier_lists.push(tier_list);
            }
        }
    }

    Ok(user_tier_lists)
}


async fn create_user(
    db: Arc<Database>,
    user_id: i64
) -> error::Result<Vec<TierList>> {
    let users = db.collection::<Document>("users");

    let user = doc! {
        "tier_lists": [],
        "user_id": user_id,
    };

    users.insert_one(user).await?;

    Ok(vec![])
}

async fn get_user_projects_response(
    db: Arc<Database>,
    user_opt: Option<Document>,
    template_link: String,
    user_id: i64
) -> error::Result<Vec<TierList>> {
    if let Some(user) = user_opt {
        query_user_projects(db, user, template_link).await
    } else {
        create_user(db, user_id).await
    }
}


pub async fn get_user_projects(
    State(db): State<Arc<Database>>,
    Json(payload): Json<GetProjectsRequest>,
) -> Result<Json<Option<Vec<TierList>>>, StatusCode> {
    let users = db.collection::<Document>("users");

    let user_opt = users
        .find_one(doc! { "user_id": payload.user_id })
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let tier_lists = get_user_projects_response(db, user_opt, payload.template_link, payload.user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(Some(tier_lists)))
}
