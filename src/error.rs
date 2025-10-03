use http::StatusCode;
use mongodb::bson::document::ValueAccessError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SharedTierListError {

    #[error("MongoDB value access failed")]
    MongoValueAccess(#[from] ValueAccessError),

    #[error("MongoDB generic error")]
    MongoError(#[from] mongodb::error::Error),

    #[error("{0}")]
    StatusCodeError(StatusCode),
}

pub type Result<T> = std::result::Result<T, SharedTierListError>;
