mod error;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::{
    http::StatusCode,
    routing::get,
    Json, Router,
};
use dotenv::dotenv;
use mongodb::bson::oid::ObjectId;
use mongodb::bson::{Array, Document};
use mongodb::{bson::doc, options::ClientOptions, Client, Collection, Database};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use axum::routing::post;

#[derive(Deserialize, Debug)]
struct TemplateInfo {
    user_id: i64,
    template_link: String
}

#[derive(Debug, Serialize, Deserialize)]
struct UserTierList {
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

fn get_string_field(
    doc: &Document,
    field_name: &str
) -> String {
    doc.get_str(field_name).expect("field must be string").to_string()
}

async fn query_tier_list(
    id: ObjectId,
    tier_lists: &Collection<Document>
) -> error::Result<Option<UserTierList>> {
    let tier_list_opt = tier_lists.find_one(doc! { "_id": id }).await?;

    if let Some(tier_list) = tier_list_opt {
        Ok(Some(UserTierList {
            id,
            name: get_string_field(&tier_list, "name"),
            template_link: get_string_field(&tier_list, "template_link"),
            tier_rows_html: get_string_field(&tier_list, "tier_rows_html"),
            tier_unused_characters_html: get_string_field(&tier_list, "tier_unused_characters_html"),
        }))
    } else {
        Ok(None)
    }
}

async fn query_user_projects(
    db: Arc<Database>,
    user: Document,
    template_link: String
) -> error::Result<Vec<UserTierList>> {
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
) -> error::Result<()> {
    let users = db.collection::<Document>("users");

    let user = doc! {
        "tier_lists": [],
        "user_id": user_id,
    };

    users.insert_one(user).await?;

    Ok(())
}

async fn get_user_projects_response(
    db: Arc<Database>,
    user_opt: Option<Document>,
    template_link: String,
    user_id: i64
) -> (StatusCode, Json<Option<Vec<UserTierList>>>) {

    if let Some(user) = user_opt {
        return match query_user_projects(db, user, template_link).await {
            Ok(lists) => {
                (StatusCode::OK, Json(Some(lists)))
            }
            Err(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(None))
            }
        }
    }

    match create_user(db, user_id).await {
        Ok(_) => {
            (StatusCode::CREATED, Json(None))
        }
        Err(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(None))
        }
    }
}

async fn get_user_projects(
    State(db): State<Arc<Database>>,
    Json(payload): Json<TemplateInfo>
) -> impl IntoResponse {
    let users = db.collection::<Document>("users");

    match users.find_one(doc! { "user_id": payload.user_id }).await {
        Ok(user_opt) => {
            get_user_projects_response(db, user_opt, payload.template_link, payload.user_id).await.into_response()
        }
        Err(error) => {
            (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
        }
    }
}

#[tokio::main]
async fn main() -> mongodb::error::Result<()> {
    dotenv().ok();

    let uri = env::var("MONGODB_URI").expect("Error: No MONGODB_URI");
    let client_options = ClientOptions::parse(uri).await?;
    let client = Client::with_options(client_options)?;
    let db = client.database("shared_tier_lists");

    let app = Router::new()
        .route("/tier-lists", get(get_user_projects))
        .with_state(Arc::new(db));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
