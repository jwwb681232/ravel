//! Ravel DB SeaORM — SeaORM database backend for Ravel.
//!
//! # Quick start
//!
//! ```rust,ignore
//! use ravel_db_seaorm::connection::ConnectionManager;
//! use ravel_db_seaorm::sea_orm::*;
//!
//! let manager = ConnectionManager::from_config("config").unwrap();
//! let db = manager.connect("default").await.unwrap();
//! ```

pub mod connection;
pub mod migration;
pub mod model;
pub mod page;
pub mod pagination;
pub mod schema;
pub mod traits;

// Re-export SeaORM for convenience.
pub use sea_orm;
pub use sea_orm_migration;

// Re-export traits and types for convenient access.
pub use traits::ConnectionManager as ConnectionManagerTrait;
pub use traits::MigrationRunner as MigrationRunnerTrait;
pub use page::Page;
