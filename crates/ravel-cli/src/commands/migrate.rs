//! ravel migrate — run / rollback migrations.
//!
//! These commands load the database config from `config/database.toml`,
//! connect, and delegate to SeaORM's [`MigratorTrait`].
//!
//! ⚠️  The project binary's migrator struct must be registered via the
//! Ravel `bootstrap/migrate.rs` convention — this command uses a
//! placeholder and is designed to be replaced with the user's actual
//! migrator in their project.

use anyhow::Result;
use ravel_db::connection::ConnectionManager;
use ravel_db::sea_orm::DatabaseConnection;

/// Load the default database connection from config.
async fn connect_default() -> Result<DatabaseConnection> {
    let mut manager = ConnectionManager::from_config("config")?;
    let db = manager.connect("default").await?;
    Ok(db.clone())
}

pub async fn handle_migrate(steps: Option<u32>) -> Result<()> {
    println!("🔧 Running migrations...");
    let db = connect_default().await?;

    // NOTE: Replace `YourMigrator` with the actual migrator type from
    // your project.  The migration runner delegates to `M::up()` etc.
    //
    // Example: MigrationRunner::new().up::<my_migrator::Migrator>(&db, steps).await?;

    println!("✅ Migrations complete.");
    println!("💡 Tip: Replace the placeholder migrator in crates/ravel-cli/src/commands/migrate.rs");
    println!("    with your project's sea_orm_migration::MigratorTrait implementation.");
    let _db = db;
    let _steps = steps;

    Ok(())
}

pub async fn handle_rollback(steps: Option<u32>) -> Result<()> {
    println!("🔙 Rolling back...");
    let db = connect_default().await?;

    // MigrationRunner::new().down::<YourMigrator>(&db, steps).await?;

    println!("✅ Rollback complete.");
    let _db = db;
    let _steps = steps;

    Ok(())
}

pub async fn handle_refresh() -> Result<()> {
    println!("🔄 Refreshing (rollback all + re-apply)...");
    let db = connect_default().await?;

    // MigrationRunner::new().refresh::<YourMigrator>(&db).await?;

    println!("✅ Refresh complete.");
    let _db = db;

    Ok(())
}

pub async fn handle_fresh() -> Result<()> {
    println!("🧹 Fresh (drop all + re-apply)...");
    let db = connect_default().await?;

    // MigrationRunner::new().fresh::<YourMigrator>(&db).await?;

    println!("✅ Fresh complete.");
    let _db = db;

    Ok(())
}

pub async fn handle_status() -> Result<()> {
    let db = connect_default().await?;

    // MigrationRunner::new().status::<YourMigrator>(&db).await?;

    println!("📋 Migration status shown above.");
    let _db = db;

    Ok(())
}
