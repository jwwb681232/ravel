//! Migration runner trait.
//!
//! Defines [`MigrationRunner`] — implement this to plug in a migration
//! backend (SeaORM, refinery, custom SQL, etc.).

use anyhow::Result;
use async_trait::async_trait;

/// Trait for running database migrations.
///
/// # Type parameters
///
/// - `Conn`: the connection type (e.g. `sea_orm::DatabaseConnection`)
#[async_trait]
pub trait MigrationRunner: Send + Sync {
    /// The connection type.
    type Conn;

    /// Run pending migrations, up to `steps` (or all if `None`).
    async fn up(&self, db: &Self::Conn, steps: Option<u32>) -> Result<()>;

    /// Rollback the last `steps` migrations (or 1 if `None`).
    async fn down(&self, db: &Self::Conn, steps: Option<u32>) -> Result<()>;

    /// Show migration status.
    async fn status(&self, db: &Self::Conn) -> Result<()>;

    /// Drop all tables and re-apply all migrations.
    async fn fresh(&self, db: &Self::Conn) -> Result<()>;

    /// Rollback all and re-apply.
    async fn refresh(&self, db: &Self::Conn) -> Result<()>;

    /// Rollback all migrations.
    async fn reset(&self, db: &Self::Conn) -> Result<()>;
}
