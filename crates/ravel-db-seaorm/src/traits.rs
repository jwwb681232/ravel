//! Database connection and migration traits.
//!
//! These were originally in a separate `ravel-db-core` crate but have been
//! inlined here since SeaORM is the only backend Ravel uses.

use anyhow::Result;
use async_trait::async_trait;

/// Trait for database connection managers.
#[async_trait]
pub trait ConnectionManager: Send + Sync {
    type Conn: Send + Sync + Clone;

    async fn connect(&self, name: &str) -> Result<Self::Conn>;
    fn config_names(&self) -> Vec<&str>;
    fn has_config(&self, name: &str) -> bool;
}

/// Trait for running database migrations.
#[async_trait]
pub trait MigrationRunner: Send + Sync {
    type Conn;

    async fn up(&self, db: &Self::Conn, steps: Option<u32>) -> Result<()>;
    async fn down(&self, db: &Self::Conn, steps: Option<u32>) -> Result<()>;
    async fn status(&self, db: &Self::Conn) -> Result<()>;
    async fn fresh(&self, db: &Self::Conn) -> Result<()>;
    async fn refresh(&self, db: &Self::Conn) -> Result<()>;
    async fn reset(&self, db: &Self::Conn) -> Result<()>;
}
