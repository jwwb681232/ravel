//! Eloquent-style ORM for Ravel.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_eloquent::Model;
//!
//! #[derive(Model)]
//! #[model(table = "users")]
//! struct User {
//!     #[model(id)] id: i32,
//!     #[model(string, 255)] name: String,
//!     #[model(string, 255, unique)] email: String,
//!     #[model(hidden)] password: String,
//!     #[model(datetime)] created_at: chrono::NaiveDateTime,
//!
//!     // Relations
//!     #[model(has_many)]
//!     pub posts: ravel_eloquent::HasMany<Post>,
//!
//!     // Many-to-many via pivot table
//!     #[model(has_many, via = "role_user")]
//!     pub roles: ravel_eloquent::BelongsToMany<Role>,
//! }
//! ```

pub mod error;
pub use error::{RavelEloquentError, Result};

pub mod fillable;
pub mod model_traits;
mod query;
mod relations;

pub use fillable::Fillable;
pub use model_traits::{
    ActiveModelExt, HasTimestamps, ModelExt, ModelMeta, Replicates, Serializes,
};
pub use query::{Page, QueryBuilder, Scope};
pub use ravel_eloquent_macros::Model;
pub use relations::{
    BelongsTo, BelongsToMany, HasMany, HasOne, RelationKind, RelationMeta, RelationQuery,
};
pub use sea_orm;
