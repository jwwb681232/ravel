use thiserror::Error;

#[derive(Error, Debug)]
pub enum RavelEloquentError {
    #[error("record not found for table '{table}' with id '{id}'")]
    RecordNotFound { table: &'static str, id: String },

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

pub type Result<T> = std::result::Result<T, RavelEloquentError>;
