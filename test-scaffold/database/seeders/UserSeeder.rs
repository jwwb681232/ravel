use anyhow::Result;
use sea_orm::DatabaseConnection;

/// Seeder: create sample users
pub async fn run(db: &DatabaseConnection) -> Result<()> {
    tracing::info!("UserSeeder: seeding started");

    db.execute_unprepared(
        "INSERT INTO users (name, email, password, created_at, updated_at) \
         VALUES ('Alice', 'alice@example.com', 'hashed_password', datetime('now'), datetime('now'))"
    ).await?;

    tracing::info!("UserSeeder: seeding complete");
    Ok(())
}
