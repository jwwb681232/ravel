//! Ravel DB SeaORM — SeaORM database backend for Ravel.
//!
//! Implements the [`ravel_db_core`] traits using SeaORM 2.0.
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
pub mod pagination;

// Re-export SeaORM for convenience.
pub use sea_orm;
pub use sea_orm_migration;

// Re-export core traits for convenient access.
pub use ravel_db_core::connection::ConnectionManager as ConnectionManagerTrait;
pub use ravel_db_core::migration::MigrationRunner as MigrationRunnerTrait;
