//! Migration runner — execute SeaORM migrations with a simple API.
//!
//! Users define a struct implementing [`sea_orm_migration::MigratorTrait`]
//! and register it here.  All the CLI commands (`ravel migrate`,
//! `ravel migrate:rollback`, etc.) delegate to this runner.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_db::migration::MigrationRunner;
//! use ravel_db::sea_orm_migration::MigratorTrait;
//!
//! struct Migrator;
//!
//! #[async_trait::async_trait]
//! impl MigratorTrait for Migrator {
//!     fn migrations() -> Vec<Box<dyn MigrationTrait>> {
//!         vec![Box::new(m20230101_000001_create_users::Migration)]
//!     }
//! }
//!
//! let runner = MigrationRunner::new();
//! runner.up::<Migrator>(&db, None).await?;
//! ```

use anyhow::Result;
use sea_orm::DatabaseConnection;
use sea_orm_migration::MigratorTrait;

/// A thin wrapper over SeaORM's migration framework.
///
/// All operations are generic over `M: MigratorTrait` — pass your
/// application's migrator struct as the type parameter.
pub struct MigrationRunner;

impl MigrationRunner {
    pub fn new() -> Self {
        Self
    }

    /// Run pending migrations up to `steps` (or all if `None`).
    pub async fn up<M: MigratorTrait>(
        &self,
        db: &DatabaseConnection,
        steps: Option<u32>,
    ) -> Result<()> {
        M::up(db, steps).await?;
        Ok(())
    }

    /// Rollback the last `steps` migrations (or 1 if `None`).
    pub async fn down<M: MigratorTrait>(
        &self,
        db: &DatabaseConnection,
        steps: Option<u32>,
    ) -> Result<()> {
        M::down(db, steps).await?;
        Ok(())
    }

    /// Show migration status.
    pub async fn status<M: MigratorTrait>(
        &self,
        db: &DatabaseConnection,
    ) -> Result<()> {
        M::status(db).await?;
        Ok(())
    }

    /// Drop all tables and re-apply all migrations.
    pub async fn fresh<M: MigratorTrait>(
        &self,
        db: &DatabaseConnection,
    ) -> Result<()> {
        M::fresh(db).await?;
        Ok(())
    }

    /// Rollback all and re-apply.
    pub async fn refresh<M: MigratorTrait>(
        &self,
        db: &DatabaseConnection,
    ) -> Result<()> {
        M::refresh(db).await?;
        Ok(())
    }

    /// Rollback all migrations.
    pub async fn reset<M: MigratorTrait>(
        &self,
        db: &DatabaseConnection,
    ) -> Result<()> {
        M::reset(db).await?;
        Ok(())
    }
}

impl Default for MigrationRunner {
    fn default() -> Self {
        Self::new()
    }
}
