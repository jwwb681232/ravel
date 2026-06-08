//! Ravel DB — Database layer built on SeaORM 2.0.
//!
//! Provides:
//! - Connection Manager — multi-connection management from `config/database.toml`
//! - Migration support — run and rollback migrations
//! - Model convenience re-exports
//! - Pagination wrapper
//! - Seeder support
//!
//! # Quick start
//!
//! ```rust,ignore
//! use ravel_db::ConnectionManager;
//! use ravel_db::sea_orm::*;
//!
//! let manager = ConnectionManager::from_config("config").unwrap();
//! let db = manager.connect("default").await.unwrap();
//!
//! // Use SeaORM normally
//! let cakes: Vec<cake::Model> = cake::Entity::find().all(&db).await.unwrap();
//! ```

pub mod connection;
pub mod model;
pub mod migration;
pub mod pagination;

// Re-export SeaORM for convenience — users can do `use ravel_db::sea_orm::*`.
pub use sea_orm;
pub use sea_orm_migration;
