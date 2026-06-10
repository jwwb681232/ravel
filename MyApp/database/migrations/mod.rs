use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            // Register new migrations here:
            // Box::new(m20240101_000001_create_users::Migration),
        ]
    }
}
