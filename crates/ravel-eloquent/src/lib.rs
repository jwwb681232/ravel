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
//!     #[model(timestamps)] created_at: chrono::NaiveDateTime,
//! }
//! ```

mod query;
mod relations;

pub use query::{ModelExt, ModelQuery, Page};
pub use ravel_eloquent_macros::Model;
pub use relations::{HasRelations, RelatedModel, RelationBuilder};
pub use sea_orm;
