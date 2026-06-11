use anyhow::Result;
use sea_orm::DatabaseConnection;

/// Seeder: {{name}}
pub async fn run(db: &DatabaseConnection) -> Result<()> {
    tracing::info!("{{name}}: seeding started");

    // Example insert (replace with your seed data):
    // db.execute_unprepared(
    //     "INSERT INTO {{snake}}s (name, created_at, updated_at) \
    //      VALUES ('example', datetime('now'), datetime('now'))"
    // ).await?;

    tracing::info!("{{name}}: seeding complete");
    Ok(())
}
