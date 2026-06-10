use thiserror::Error;
use ravel_error::{ErrorKind, HttpError};
use std::collections::HashMap;

#[derive(Error, Debug)]
pub enum RavelEloquentError {
    #[error("record not found for table '{table}' with id '{id}'")]
    RecordNotFound { table: &'static str, id: String },

    #[error("validation failed: {errors:?}")]
    ValidationError { errors: HashMap<String, Vec<String>> },

    #[error("invalid column '{column}' for table '{table}'")]
    InvalidColumn { table: &'static str, column: String },

    #[error("cannot update record with id = 0; use save() or insert() instead")]
    UpdateWithoutId,

    #[error("cannot insert record with non-zero id; use save() or update() instead")]
    InsertWithId,

    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

impl HttpError for RavelEloquentError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::RecordNotFound { .. } => ErrorKind::NotFound,
            Self::ValidationError { .. } => ErrorKind::Validation,
            Self::InvalidColumn { .. }
            | Self::UpdateWithoutId
            | Self::InsertWithId => ErrorKind::BadRequest,
            Self::Database(_) | Self::Serialization(_) | Self::Other(_) => ErrorKind::Internal,
        }
    }
}

pub type Result<T> = std::result::Result<T, RavelEloquentError>;
