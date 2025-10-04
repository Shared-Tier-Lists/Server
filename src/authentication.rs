use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::db_constants::{Collections, UserFields};
use crate::AppState;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2
};
use chrono::{Duration, Utc};
use headers::authorization::Bearer;
use headers::Authorization;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use mongodb::bson::oid::ObjectId;
use mongodb::bson::{doc, Document};
use password_hash::rand_core::OsRng;
use sha2::Digest;

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: i64,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    email: String,
    display_name: String,
    password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    token: String,
}

pub async fn signup(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> Result<StatusCode, StatusCode> {
    let users = state.db.collection::<Document>(Collections::USERS);
    let user_opt: Option<Document> = users
        .find_one(doc! { UserFields::EMAIL: payload.email.clone() })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match user_opt {
        Some(_) => Err(StatusCode::CONFLICT),
        None => {
            let argon2 = Argon2::default();
            let salt = SaltString::generate(OsRng);
            let password_hash = argon2.hash_password(payload.password.as_bytes(), &salt)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                .to_string();

            users.insert_one(doc! {
                UserFields::EMAIL: payload.email,
                UserFields::DISPLAY_NAME: payload.display_name,
                UserFields::PASSWD_HASH: password_hash,
                UserFields::PROJECTS: [],
            }).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            Ok(StatusCode::CREATED)
        }
    }
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    let users = state.db.collection::<Document>(Collections::USERS);
    let user_opt: Option<Document> = users.find_one(doc! { UserFields::EMAIL: payload.email.clone() }).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user = match user_opt {
        Some(user_doc) => user_doc,
        None => return Err(StatusCode::UNAUTHORIZED)
    };

    let stored_hash = user.get_str(UserFields::PASSWD_HASH)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let parsed_hash = PasswordHash::new(stored_hash)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let argon2 = Argon2::default();
    if argon2.verify_password(payload.password.as_bytes(), &parsed_hash).is_err() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let expiration = Utc::now()
        .checked_add_signed(Duration::minutes(15))
        .unwrap()
        .timestamp();

    let claims = Claims {
        sub: user.get_str(UserFields::ID)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.to_string(),
        exp: expiration,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret_key.as_ref())
    ).unwrap();

    Ok(Json(LoginResponse { token }))
}

pub async fn authenticate_user(
    user_id: ObjectId,
    app_state: Arc<AppState>,
    auth: Authorization<Bearer>,
) -> Result<Document, StatusCode> {
     let claims = decode::<Claims>(
         auth.token(),
         &DecodingKey::from_secret(app_state.jwt_secret_key.as_ref()),
         &Validation::default()
    ).map_err(|_| StatusCode::UNAUTHORIZED)?;

    if user_id.to_hex() != claims.claims.sub {
        return Err(StatusCode::UNAUTHORIZED)
    }

    let users = app_state.db.collection::<Document>(Collections::USERS);

    let user_opt = users
        .find_one(doc! { UserFields::ID: user_id })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match user_opt {
        Some(user) => Ok(user),
        None => Err(StatusCode::UNAUTHORIZED)
    }
}
