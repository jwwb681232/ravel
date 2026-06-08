//! Ravel Generator — template-based code scaffolding engine.
//!
//! Generates files for:
//! - Controllers
//! - Middleware
//! - Migrations
//! - Seeders
//! - Providers
//! - FormRequests
//!
//! Templates use `{{variable}}` placeholders. Available variables:
//! - `{{name}}` — original CamelCase name
//! - `{{snake}}` — snake_case version
//! - `{{kebab}}` — kebab-case version
//! - `{{timestamp}}` — current UTC timestamp (for migrations)

use anyhow::{bail, Result};
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

// ── Generator ─────────────────────────────────────────────────────

/// The scaffolding engine, rooted at a project directory.
pub struct Generator {
    root: PathBuf,
}

impl Generator {
    /// Create a generator targeting `root` as the project root.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Return the project root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Ensure a sub-directory exists.
    pub fn ensure_dir(&self, rel: &str) -> Result<PathBuf> {
        let dir = self.root.join(rel);
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Check whether a file exists relative to the project root.
    pub fn exists(&self, rel: &str) -> bool {
        self.root.join(rel).exists()
    }

    /// Write a file, failing if it already exists.
    pub fn create_file(&self, rel: &str, content: &str) -> Result<()> {
        let path = self.root.join(rel);
        if path.exists() {
            bail!("File '{}' already exists", rel);
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;
        Ok(())
    }

    /// Write a file, overwriting if it exists.
    pub fn overwrite_file(&self, rel: &str, content: &str) -> Result<()> {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;
        Ok(())
    }

    // ── Template variables ──────────────────────────────────────

    /// Render `template` with the given name-derived variables.
    pub fn render(&self, template: &str, name: &str) -> String {
        let snake = to_snake(name);
        let kebab = to_kebab(name);
        let timestamp = Utc::now().format("%Y_%m_%d_%H%M%S").to_string();

        template
            .replace("{{timestamp}}", &timestamp)
            .replace("{{name}}", name)
            .replace("{{snake}}", &snake)
            .replace("{{kebab}}", &kebab)
    }

    // ── Built-in scaffolds ──────────────────────────────────────

    /// Generate a Controller file.
    pub fn scaffold_controller(&self, name: &str) -> Result<()> {
        let content = self.render(CONTROLLER_TEMPLATE, name);
        self.create_file(&format!("app/Http/Controllers/{name}.rs"), &content)
    }

    /// Generate a Middleware file.
    pub fn scaffold_middleware(&self, name: &str) -> Result<()> {
        let content = self.render(MIDDLEWARE_TEMPLATE, name);
        self.create_file(&format!("app/Http/Middleware/{name}.rs"), &content)
    }

    /// Generate a Migration file (timestamped).
    pub fn scaffold_migration(&self, name: &str) -> Result<()> {
        let content = self.render(MIGRATION_TEMPLATE, name);
        let timestamp = Utc::now().format("%Y_%m_%d_%H%M%S");
        let snake = to_snake(name);
        self.create_file(
            &format!("database/migrations/{}_{}.rs", timestamp, snake),
            &content,
        )
    }

    /// Generate a Seeder file.
    pub fn scaffold_seeder(&self, name: &str) -> Result<()> {
        let content = self.render(SEEDER_TEMPLATE, name);
        self.create_file(&format!("database/seeders/{name}.rs"), &content)
    }

    /// Generate a ServiceProvider file.
    pub fn scaffold_provider(&self, name: &str) -> Result<()> {
        let content = self.render(PROVIDER_TEMPLATE, name);
        self.create_file(&format!("app/Providers/{name}.rs"), &content)
    }

    /// Generate a FormRequest file.
    pub fn scaffold_request(&self, name: &str) -> Result<()> {
        let content = self.render(REQUEST_TEMPLATE, name);
        self.create_file(&format!("app/Http/Requests/{name}.rs"), &content)
    }

    /// Generate a Model file (SeaORM entity).
    pub fn scaffold_model(&self, name: &str) -> Result<()> {
        let content = self.render(MODEL_TEMPLATE, name);
        self.create_file(&format!("app/Models/{name}.rs"), &content)
    }

    /// Scaffold the initial project skeleton (used by `ravel new`).
    pub fn scaffold_project(&self, project_name: &str) -> Result<()> {
        let dirs = [
            "app/Http/Controllers",
            "app/Http/Middleware",
            "app/Http/Requests",
            "app/Models",
            "app/Services",
            "app/Providers",
            "bootstrap",
            "config",
            "database/migrations",
            "database/seeders",
            "routes",
            "storage",
            "tests",
            "src",
            "src/bin",
        ];

        for dir in dirs {
            self.ensure_dir(dir)?;
        }

        // Cargo.toml
        self.overwrite_file(
            "Cargo.toml",
            &self.render(CARGO_TOML_TEMPLATE, project_name),
        )?;

        // src/main.rs
        self.overwrite_file("src/main.rs", &self.render(MAIN_RS_TEMPLATE, project_name))?;

        // Default config file
        self.overwrite_file("config/app.toml", APP_TOML_TEMPLATE)?;

        // Default .env
        if !self.exists(".env") {
            self.overwrite_file(".env", ENV_TEMPLATE)?;
        }

        // .env.example (always overwrite to keep in sync)
        self.overwrite_file(".env.example", ENV_TEMPLATE)?;

        // Database migrator (database/migrations/mod.rs)
        if !self.exists("database/migrations/mod.rs") {
            self.overwrite_file("database/migrations/mod.rs", MIGRATOR_TEMPLATE)?;
        }

        // Migrate binary (src/bin/migrate.rs)
        if !self.exists("src/bin/migrate.rs") {
            self.overwrite_file("src/bin/migrate.rs", MIGRATE_BIN_TEMPLATE)?;
        }

        // Seed binary (src/bin/seed.rs)
        if !self.exists("src/bin/seed.rs") {
            self.overwrite_file("src/bin/seed.rs", SEED_BIN_TEMPLATE)?;
        }

        Ok(())
    }
}

// ── Helpers ────────────────────────────────────────────────────────

fn to_snake(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_uppercase() {
            if !out.is_empty() {
                out.push('_');
            }
            out.push(c.to_lowercase().next().unwrap());
        } else {
            out.push(c);
        }
    }
    out
}

fn to_kebab(s: &str) -> String {
    to_snake(s).replace('_', "-")
}

// ── Templates ──────────────────────────────────────────────────────

const CONTROLLER_TEMPLATE: &str = r#"use ravel_http::controller::Controller;
use ravel_core::container::Container;

pub struct {{name}};

impl Controller for {{name}} {
    fn boot(_container: &Container) -> Self {
        Self
    }
}

// ── Handlers ──────────────────────────────────────────────────────
//
// impl {{name}} {
//     pub async fn index() -> impl axum::response::IntoResponse {
//         "{{name}} index"
//     }
// }
"#;

const MIDDLEWARE_TEMPLATE: &str = r#"use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};

/// {{name}} — custom middleware.
pub struct {{name}};

impl {{name}} {
    pub fn new() -> Self {
        Self
    }
}

pub async fn handle(req: Request, next: Next) -> Response {
    // TODO: pre-processing logic here

    let response = next.run(req).await;

    // TODO: post-processing logic here

    response
}
"#;

const MIGRATION_TEMPLATE: &str = r#"use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // TODO: apply the migration
        // Example:
        // manager.create_table(
        //     Table::create()
        //         .table(Users::Table)
        //         .col(ColumnDef::new(Users::Id).integer().not_null().auto_increment().primary_key())
        //         .col(ColumnDef::new(Users::Name).string().not_null())
        //         .to_owned()
        // ).await
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // TODO: rollback the migration
        // Example:
        // manager.drop_table(Table::drop().table(Users::Table).to_owned()).await
        Ok(())
    }
}
"#;

const SEEDER_TEMPLATE: &str = r#"use anyhow::Result;
use sea_orm::DatabaseConnection;

/// Seeder: {{name}}

pub async fn run(_db: &DatabaseConnection) -> Result<()> {
    // TODO: insert seed data
    // Example:
    // use sea_orm::ActiveModelTrait;
    // use sea_orm::ActiveValue::Set;
    // let model = your_model::ActiveModel { name: Set("test".into()), ..Default::default() };
    // model.insert(_db).await?;
    Ok(())
}
"#;

const PROVIDER_TEMPLATE: &str = r#"use ravel_core::app::ServiceProvider;
use ravel_core::container::Container;
use anyhow::Result;

pub struct {{name}};

impl ServiceProvider for {{name}} {
    fn register(&self, container: &Container) -> Result<()> {
        // Bind services into the container here.
        let _ = container;
        Ok(())
    }

    fn boot(&self, container: &Container) -> Result<()> {
        // Wire up dependencies — all providers are registered at this point.
        let _ = container;
        Ok(())
    }

    fn name(&self) -> &str {
        "{{name}}"
    }
}
"#;

const REQUEST_TEMPLATE: &str = r#"use ravel_http::form_request::FormRequest;
use ravel_http::validation::{FieldRule, Rule};
use serde::Deserialize;

/// Form request: {{name}}
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct {{name}} {
    // TODO: define fields
    // pub name: String,
    // pub email: String,
}

impl FormRequest for {{name}} {
    fn rules() -> Vec<FieldRule> {
        vec![
            // FieldRule::new("name", vec![Rule::Required, Rule::Min(3)]),
            // FieldRule::new("email", vec![Rule::Required, Rule::Email]),
        ]
    }

    // fn authorize(&self) -> bool {
    //     true
    // }

    // fn messages() -> std::collections::HashMap<String, String> {
    //     let mut m = std::collections::HashMap::new();
    //     m.insert("name.required".into(), "Name is required".into());
    //     m
    // }
}
"#;

const MODEL_TEMPLATE: &str = r#"use sea_orm::entity::prelude::*;

/// Model: {{name}}
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "{{snake}}")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    // TODO: add columns
    // pub name: String,
    // pub email: String,
    // pub created_at: DateTime,
    // pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
"#;

const CARGO_TOML_TEMPLATE: &str = r#"[package]
name = "{{snake}}"
version = "0.1.0"
edition = "2024"

[dependencies]
ravel-core = { path = "../ravel-core" }
ravel-http = { path = "../ravel-http" }
axum = "0.8"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
"#;

const MAIN_RS_TEMPLATE: &str = r#"use axum::Router;
use ravel_http::route::Route;

#[tokio::main]
async fn main() {
    let app: Router = Route::new()
        .get("/", || async { "Hello, Ravel! 🚀" })
        .build();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("Ravel running at http://127.0.0.1:3000");

    axum::serve(listener, app).await.unwrap();
}
"#;

const APP_TOML_TEMPLATE: &str = r#"# Ravel application configuration

[app]
name = "{{name}}"
env = "local"
debug = true
url = "http://localhost:3000"

[server]
host = "127.0.0.1"
port = 3000
"#;

const ENV_TEMPLATE: &str = r#"APP_NAME={{name}}
APP_ENV=local
APP_DEBUG=true
APP_URL=http://localhost:3000
"#;

const MIGRATOR_TEMPLATE: &str = r#"use sea_orm_migration::prelude::*;

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
"#;

const MIGRATE_BIN_TEMPLATE: &str = r#"use std::env;
use sea_orm::Database;
use sea_orm_migration::MigratorTrait;
use ravel_db::connection::ConnectionManager;

#[path = "../../database/migrations/mod.rs"]
mod migrations;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut manager = ConnectionManager::from_config("config")?;
    let db = manager.connect("default").await?;

    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("up");
    let steps: Option<u32> = args.get(2).and_then(|s| s.parse().ok());

    match cmd {
        "up" | "migrate" => {
            println!("Running migrations...");
            migrations::Migrator::up(db, steps).await?;
            println!("Migrations complete.");
        }
        "down" | "rollback" => {
            println!("Rolling back...");
            migrations::Migrator::down(db, steps.or(Some(1))).await?;
            println!("Rollback complete.");
        }
        "fresh" => {
            println!("Dropping all tables and re-applying...");
            migrations::Migrator::fresh(db).await?;
            println!("Fresh complete.");
        }
        "refresh" => {
            println!("Refreshing (rollback all + re-apply)...");
            migrations::Migrator::refresh(db).await?;
            println!("Refresh complete.");
        }
        "status" => {
            migrations::Migrator::status(db).await?;
        }
        other => {
            eprintln!("Unknown command: {other}");
            eprintln!("Usage: migrate [up|down|fresh|refresh|status] [steps]");
            std::process::exit(1);
        }
    }

    Ok(())
}
"#;

const SEED_BIN_TEMPLATE: &str = r#"use sea_orm::Database;
use ravel_db::connection::ConnectionManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut manager = ConnectionManager::from_config("config")?;
    let db = manager.connect("default").await?;

    println!("Seeding database...");

    // Register and run seeders here:
    // seeders::UserSeeder::run(db).await?;

    println!("Database seeded.");
    Ok(())
}
"#;

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snake_case() {
        assert_eq!(to_snake("HelloWorld"), "hello_world");
        assert_eq!(to_snake("HTTPS"), "h_t_t_p_s");
        assert_eq!(to_snake("already_snake"), "already_snake");
        assert_eq!(to_snake("UserController"), "user_controller");
    }

    #[test]
    fn test_kebab_case() {
        assert_eq!(to_kebab("HelloWorld"), "hello-world");
        assert_eq!(to_kebab("UserController"), "user-controller");
    }

    #[test]
    fn test_render() {
        let g = Generator::new("/tmp/test");
        let output = g.render("Hello {{name}}, your file is {{snake}}.rs", "UserController");
        assert!(output.contains("UserController"));
        assert!(output.contains("user_controller.rs"));
        assert!(!output.contains("{{name}}"));
    }

    #[test]
    fn test_generate_controller() {
        let tmp = std::env::temp_dir().join("ravel_gen_test");
        let _ = std::fs::remove_dir_all(&tmp);

        let g = Generator::new(&tmp);
        g.scaffold_controller("UserController").unwrap();

        let content = std::fs::read_to_string(tmp.join("app/Http/Controllers/UserController.rs")).unwrap();
        assert!(content.contains("pub struct UserController"));
        assert!(content.contains("impl Controller for UserController"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_generate_middleware() {
        let tmp = std::env::temp_dir().join("ravel_gen_test2");
        let _ = std::fs::remove_dir_all(&tmp);

        let g = Generator::new(&tmp);
        g.scaffold_middleware("AuthMiddleware").unwrap();

        let content = std::fs::read_to_string(tmp.join("app/Http/Middleware/AuthMiddleware.rs")).unwrap();
        assert!(content.contains("pub struct AuthMiddleware"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_generate_migration() {
        let tmp = std::env::temp_dir().join("ravel_gen_test3");
        let _ = std::fs::remove_dir_all(&tmp);

        let g = Generator::new(&tmp);
        g.scaffold_migration("CreateUsersTable").unwrap();

        let dir = tmp.join("database/migrations");
        let entries: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        assert_eq!(entries.len(), 1);

        let path = entries[0].as_ref().unwrap().path();
        let fname = path.file_name().unwrap().to_str().unwrap();
        assert!(fname.ends_with("_create_users_table.rs"));

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("MigrationTrait"));
        assert!(content.contains("async fn up"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_generate_seeder() {
        let tmp = std::env::temp_dir().join("ravel_gen_test4");
        let _ = std::fs::remove_dir_all(&tmp);

        let g = Generator::new(&tmp);
        g.scaffold_seeder("UserSeeder").unwrap();

        let content = std::fs::read_to_string(tmp.join("database/seeders/UserSeeder.rs")).unwrap();
        assert!(content.contains("pub async fn run(_db: &DatabaseConnection)"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_generate_provider() {
        let tmp = std::env::temp_dir().join("ravel_gen_test5");
        let _ = std::fs::remove_dir_all(&tmp);

        let g = Generator::new(&tmp);
        g.scaffold_provider("RouteServiceProvider").unwrap();

        let content = std::fs::read_to_string(tmp.join("app/Providers/RouteServiceProvider.rs")).unwrap();
        assert!(content.contains("impl ServiceProvider for RouteServiceProvider"));
        assert!(content.contains("fn register"));
        assert!(content.contains("fn boot"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_generate_request() {
        let tmp = std::env::temp_dir().join("ravel_gen_test6");
        let _ = std::fs::remove_dir_all(&tmp);

        let g = Generator::new(&tmp);
        g.scaffold_request("LoginRequest").unwrap();

        let content = std::fs::read_to_string(tmp.join("app/Http/Requests/LoginRequest.rs")).unwrap();
        assert!(content.contains("pub struct LoginRequest"));
        assert!(content.contains("impl FormRequest for LoginRequest"));
        assert!(content.contains("fn rules"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_scaffold_project() {
        let tmp = std::env::temp_dir().join("ravel_gen_project");
        let _ = std::fs::remove_dir_all(&tmp);

        let g = Generator::new(&tmp);
        g.scaffold_project("MyApp").unwrap();

        assert!(tmp.join("Cargo.toml").exists());
        assert!(tmp.join("src/main.rs").exists());
        assert!(tmp.join("config/app.toml").exists());
        assert!(tmp.join(".env").exists());
        assert!(tmp.join("app/Http/Controllers").is_dir());
        assert!(tmp.join("database/migrations").is_dir());
        assert!(tmp.join("database/migrations/mod.rs").exists());
        assert!(tmp.join("src/bin/migrate.rs").exists());
        assert!(tmp.join("src/bin/seed.rs").exists());

        let cargo = std::fs::read_to_string(tmp.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("my_app"));

        let migrator = std::fs::read_to_string(tmp.join("database/migrations/mod.rs")).unwrap();
        assert!(migrator.contains("MigratorTrait"));

        let migrate_bin = std::fs::read_to_string(tmp.join("src/bin/migrate.rs")).unwrap();
        assert!(migrate_bin.contains("Migrator::up"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
