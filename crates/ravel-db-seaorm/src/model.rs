//! Model convenience layer — re-exports and extension traits.
//!
//! Re-exports SeaORM's core derive macros and entity types so users can:
//!
//! ```rust,ignore
//! use ravel_db::model::*;
//! use ravel_db::sea_orm::entity::prelude::*;
//!
//! #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
//! #[sea_orm(table_name = "users")]
//! pub struct Model {
//!     #[sea_orm(primary_key)]
//!     pub id: i32,
//!     pub name: String,
//!     pub email: String,
//! }
//!
//! #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
//! pub enum Relation {}
//!
//! impl ActiveModelBehavior for ActiveModel {}
//! ```

// Re-export the key SeaORM macros so users can `use ravel_db::model::*`.
pub use sea_orm::{
    ActiveModelBehavior, ActiveModelTrait, ActiveValue, ColumnTrait, DeriveActiveModel,
    DeriveColumn, DeriveEntityModel, DeriveIntoActiveModel, DerivePartialModel, DerivePrimaryKey,
    DeriveRelation, EntityTrait, EnumIter, ModelTrait, PaginatorTrait, PrimaryKeyTrait,
    QueryFilter, QueryOrder, QuerySelect, Related, RelationTrait,
};
