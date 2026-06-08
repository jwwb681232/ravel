# Migrations

Ravel uses SeaORM's migration framework for managing database schema changes. Migrations are timestamp-ordered Rust files with `up` and `down` methods.

## CLI Commands

The `ravel` CLI provides several migration commands:

```bash
# Run pending migrations
ravel migrate

# Rollback the last migration
ravel migrate:rollback

# Rollback all and re-apply
ravel migrate:refresh

# Drop all tables and re-apply
ravel migrate:fresh

# Show migration status
ravel migrate:status
```

## Creating a Migration

Use the CLI to scaffold a migration:

```bash
ravel make:migration create_users_table
```

This generates a file in `database/migrations/` with the current timestamp:

```
database/migrations/
├── m20260101_000001_create_users_table.rs
└── mod.rs
```

## Migration Structure

Each migration defines `up` (apply) and `down` (rollback) methods:

```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Users::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Users::Name).string().not_null())
                    .col(ColumnDef::new(Users::Email).string().not_null().unique_key())
                    .col(ColumnDef::new(Users::CreatedAt).timestamp().default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Users {
    Table,
    Id,
    Name,
    Email,
    CreatedAt,
}
```

## Registering Migrations

Register all migration modules in the mod file and the migrator struct:

```rust
// database/migrations/mod.rs
pub use sea_orm_migration::prelude::*;

mod m20260101_000001_create_users_table;
pub use m20260101_000001_create_users_table::Migration as CreateUsersTable;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(CreateUsersTable),
        ]
    }
}
```

## MigrationRunner API

For programmatic control, use `MigrationRunner`:

```rust
use ravel_db_seaorm::migration::MigrationRunner;
use ravel_db_seaorm::connection::ConnectionManager;

let manager = ConnectionManager::from_config("config")?;
let db = manager.connect("default").await?;

let runner = MigrationRunner::new();

// Run all pending migrations
runner.up::<Migrator>(&db, None).await?;

// Run only the next 2
runner.up::<Migrator>(&db, Some(2)).await?;

// Rollback the last migration
runner.down::<Migrator>(&db, None).await?;

// Show status
runner.status::<Migrator>(&db).await?;

// Drop all tables and re-apply
runner.fresh::<Migrator>(&db).await?;

// Rollback all and re-apply
runner.refresh::<Migrator>(&db).await?;

// Rollback all
runner.reset::<Migrator>(&db).await?;
```

| Method       | Description |
|--------------|-------------|
| `up(db, steps)` | Run pending migrations (up to `steps` or all) |
| `down(db, steps)` | Rollback last `steps` (or 1) |
| `status(db)`  | Show which migrations have been applied |
| `fresh(db)`   | Drop all tables and re-apply everything |
| `refresh(db)` | Rollback all, then re-apply |
| `reset(db)`   | Rollback all migrations |
