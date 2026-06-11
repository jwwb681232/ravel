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

use anyhow::{Context, Result, bail};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use tera::{Context as TeraContext, Tera};

// ── Generator ─────────────────────────────────────────────────────

/// The scaffolding engine, rooted at a project directory.
pub struct Generator {
    root: PathBuf,
    tera: Tera,
}

impl Generator {
    /// Create a generator targeting `root` as the project root.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let mut tera = Tera::default();
        // Register all built-in templates
        tera.add_raw_template("controller", CONTROLLER_TEMPLATE)
            .unwrap();
        tera.add_raw_template("middleware", MIDDLEWARE_TEMPLATE)
            .unwrap();
        tera.add_raw_template("migration", MIGRATION_TEMPLATE)
            .unwrap();
        tera.add_raw_template("seeder", SEEDER_TEMPLATE).unwrap();
        tera.add_raw_template("provider", PROVIDER_TEMPLATE)
            .unwrap();
        tera.add_raw_template("request", REQUEST_TEMPLATE).unwrap();
        tera.add_raw_template("model", MODEL_TEMPLATE).unwrap();
        tera.add_raw_template("job", JOB_TEMPLATE).unwrap();
        tera.add_raw_template("cargo_toml", CARGO_TOML_TEMPLATE)
            .unwrap();
        tera.add_raw_template("cargo_toml_dev", CARGO_TOML_DEV_TEMPLATE)
            .unwrap();
        tera.add_raw_template("main_rs", MAIN_RS_TEMPLATE).unwrap();
        tera.add_raw_template("app_toml", APP_TOML_TEMPLATE)
            .unwrap();
        tera.add_raw_template("env", ENV_TEMPLATE).unwrap();
        tera.add_raw_template("migrator", MIGRATOR_TEMPLATE)
            .unwrap();
        tera.add_raw_template("migrate_bin", MIGRATE_BIN_TEMPLATE)
            .unwrap();
        tera.add_raw_template("seed_bin", SEED_BIN_TEMPLATE)
            .unwrap();
        Self {
            root: root.into(),
            tera,
        }
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

    // ── Template rendering ───────────────────────────────────────

    /// Build a Tera context with name-derived variables.
    fn make_context(name: &str) -> TeraContext {
        let snake = to_snake(name);
        let kebab = to_kebab(name);
        let timestamp = Utc::now().format("%Y_%m_%d_%H%M%S").to_string();

        let mut ctx = TeraContext::new();
        ctx.insert("name", name);
        ctx.insert("snake", &snake);
        ctx.insert("kebab", &kebab);
        ctx.insert("timestamp", &timestamp);
        ctx
    }

    /// Render a built-in template by name with the given name-derived variables.
    pub fn render(&self, template_name: &str, name: &str) -> Result<String> {
        let ctx = Self::make_context(name);
        self.tera
            .render(template_name, &ctx)
            .with_context(|| format!("Failed to render template '{template_name}'"))
    }

    // ── Built-in scaffolds ──────────────────────────────────────

    /// Generate a Controller file.
    pub fn scaffold_controller(&self, name: &str) -> Result<()> {
        let content = self.render("controller", name)?;
        self.create_file(&format!("app/Http/Controllers/{name}.rs"), &content)
    }

    /// Generate a Middleware file.
    pub fn scaffold_middleware(&self, name: &str) -> Result<()> {
        let content = self.render("middleware", name)?;
        self.create_file(&format!("app/Http/Middleware/{name}.rs"), &content)
    }

    /// Generate a Migration file (timestamped).
    pub fn scaffold_migration(&self, name: &str) -> Result<()> {
        let content = self.render("migration", name)?;
        let timestamp = Utc::now().format("%Y_%m_%d_%H%M%S");
        let snake = to_snake(name);
        self.create_file(
            &format!("database/migrations/{}_{}.rs", timestamp, snake),
            &content,
        )
    }

    /// Generate a Seeder file.
    pub fn scaffold_seeder(&self, name: &str) -> Result<()> {
        let content = self.render("seeder", name)?;
        self.create_file(&format!("database/seeders/{name}.rs"), &content)
    }

    /// Generate a ServiceProvider file.
    pub fn scaffold_provider(&self, name: &str) -> Result<()> {
        let content = self.render("provider", name)?;
        self.create_file(&format!("app/Providers/{name}.rs"), &content)
    }

    /// Generate a FormRequest file.
    pub fn scaffold_request(&self, name: &str) -> Result<()> {
        let content = self.render("request", name)?;
        self.create_file(&format!("app/Http/Requests/{name}.rs"), &content)
    }

    /// Generate a Model file (SeaORM entity).
    pub fn scaffold_model(&self, name: &str) -> Result<()> {
        let content = self.render("model", name)?;
        self.create_file(&format!("app/Models/{name}.rs"), &content)
    }

    /// Generate a Job file.
    pub fn scaffold_job(&self, name: &str) -> Result<()> {
        let content = self.render("job", name)?;
        self.create_file(&format!("app/Jobs/{name}.rs"), &content)
    }

    /// Scaffold the initial project skeleton (used by `ravel new`).
    ///
    /// When `dev` is true, Cargo.toml uses `path` dependencies
    /// pointing to the local framework crates, and a `.ravel-dev`
    /// marker file is written.
    pub fn scaffold_project(&self, project_name: &str, dev: bool) -> Result<()> {
        let dirs = [
            "app/Http/Controllers",
            "app/Http/Middleware",
            "app/Http/Requests",
            "app/Models",
            "app/Jobs",
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

        // Cargo.toml — choose template based on dev mode
        let cargo_template = if dev {
            "cargo_toml_dev"
        } else {
            "cargo_toml"
        };
        self.overwrite_file(
            "Cargo.toml",
            &self.render(cargo_template, project_name)?,
        )?;

        // src/main.rs
        self.overwrite_file("src/main.rs", &self.render("main_rs", project_name)?)?;

        // Default config file
        self.overwrite_file("config/app.toml", &self.render("app_toml", project_name)?)?;

        // Default .env
        if !self.exists(".env") {
            self.overwrite_file(".env", &self.render("env", project_name)?)?;
        }

        // .env.example (always overwrite to keep in sync)
        self.overwrite_file(".env.example", &self.render("env", project_name)?)?;

        // Dev marker file — records framework_root for ravel serve
        if dev {
            let framework_root = std::env::current_dir()
                .context("Failed to read current directory")?;
            // TOML requires forward slashes in paths
            let root_str = framework_root.display().to_string().replace('\\', "/");
            let marker = format!(
                "framework_root = \"{}\"\n",
                root_str
            );
            self.overwrite_file(".ravel-dev", &marker)?;
        }

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
    let chars = s.chars().peekable();
    for c in chars {
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
///
/// Apply via: `Route::new().middleware({{name}}::layer()).get("/", handler)`
pub struct {{name}};

impl {{name}} {
    /// Create the Axum-compatible middleware layer.
    pub fn layer() -> axum::middleware::from_fn(handle)
    where
    {
        axum::middleware::from_fn(handle)
    }
}

pub async fn handle(req: Request, next: Next) -> Response {
    // Pre-processing: runs before the handler
    tracing::info!("{{name}}: processing request {} {}", req.method(), req.uri());

    let response = next.run(req).await;

    // Post-processing: runs after the handler
    tracing::info!("{{name}}: response status {}", response.status());

    response
}
"#;

const MIGRATION_TEMPLATE: &str = r#"use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table({{name}}::Table)
                    .if_not_exists()
                    .col(ColumnDef::new({{name}}::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new({{name}}::Name).string().not_null())
                    .col(ColumnDef::new({{name}}::CreatedAt).timestamp().not_null()
                        .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)))
                    .col(ColumnDef::new({{name}}::UpdatedAt).timestamp().not_null()
                        .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table({{name}}::Table).to_owned())
            .await
    }
}

// ── Replace with your actual table definition ──────────────────
#[derive(Iden)]
enum {{name}} {
    Table,
    Id,
    Name,
    CreatedAt,
    UpdatedAt,
}
"#;

const SEEDER_TEMPLATE: &str = r#"use anyhow::Result;
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

const JOB_TEMPLATE: &str = r#"use ravel_macros::Job;
use serde::{Serialize, Deserialize};

/// Job: {{name}}
#[derive(Serialize, Deserialize, Job)]
#[job(name = "{{snake}}")]
pub struct {{name}} {
    // Add your payload fields here — example:
    // pub user_id: i32,
    // pub email: String,
}

impl {{name}} {
    /// Core job logic — called by the generated `Job::handle()`.
    pub async fn execute(&self) -> anyhow::Result<()> {
        tracing::info!("{{name}}: executing");
        // TODO: implement your job logic here

        Ok(())
    }
}
"#;

const REQUEST_TEMPLATE: &str = r#"use ravel_http::form_request::FormRequest;
use ravel_http::validation::{FieldRule, Rule};
use serde::Deserialize;

/// Form request: {{name}}
#[derive(Debug, Deserialize)]
pub struct {{name}} {
    pub name: String,
    pub email: String,
    // Add your fields here
}

impl FormRequest for {{name}} {
    fn rules() -> Vec<FieldRule> {
        vec![
            FieldRule::new("name", vec![Rule::Required, Rule::Min(3)]),
            FieldRule::new("email", vec![Rule::Required, Rule::Email]),
        ]
    }

    // Optional — uncomment to customize:
    //
    // fn authorize(&self) -> bool {
    //     self.role == "admin"
    // }
    //
    // fn messages() -> std::collections::HashMap<String, String> {
    //     let mut m = std::collections::HashMap::new();
    //     m.insert("name.required".into(), "Please enter your name".into());
    //     m
    // }
}
"#;

const MODEL_TEMPLATE: &str = r#"use ravel_eloquent::Model;
use serde::{Deserialize, Serialize};

/// Model: {{name}}
#[derive(Model, Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
#[model(table = "{{snake}}s")]
pub struct {{name}} {
    #[model(id)]
    pub id: i32,
    // Add your columns here — example:
    // pub name: String,
    // #[model(string, 254, unique)]
    // pub email: String,
    // #[model(nullable, text)]
    // pub bio: Option<String>,
}
"#;

const CARGO_TOML_TEMPLATE: &str = r#"[package]
name = "{{snake}}"
version = "0.1.0"
edition = "2024"

[dependencies]
ravel-core     = { git = "https://github.com/jwwb681232/ravel" }
ravel-http     = { git = "https://github.com/jwwb681232/ravel" }
ravel-eloquent = { git = "https://github.com/jwwb681232/ravel" }
ravel-facades  = { git = "https://github.com/jwwb681232/ravel" }
ravel-db-seaorm = { git = "https://github.com/jwwb681232/ravel" }
sea-orm             = { version = "2.0.0-rc.40", features = ["sqlx-sqlite", "runtime-tokio-rustls"] }
sea-orm-migration   = { version = "2.0.0-rc.40" }
axum   = { version = "0.8", features = ["multipart"] }
tokio  = { version = "1", features = ["full"] }
serde  = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow  = "1"
async-trait = "0.1"
"#;

const CARGO_TOML_DEV_TEMPLATE: &str = r#"[package]
name = "{{snake}}"
version = "0.1.0"
edition = "2024"

[dependencies]
ravel-core     = { path = "../crates/ravel-core" }
ravel-http     = { path = "../crates/ravel-http" }
ravel-eloquent = { path = "../crates/ravel-eloquent" }
ravel-facades  = { path = "../crates/ravel-facades" }
ravel-db-seaorm = { path = "../crates/ravel-db-seaorm" }
sea-orm             = { workspace = true, features = ["sqlx-sqlite", "runtime-tokio-rustls"] }
sea-orm-migration   = { workspace = true }
axum   = { workspace = true, features = ["multipart"] }
tokio  = { workspace = true, features = ["full"] }
serde  = { workspace = true }
serde_json = { workspace = true }
anyhow  = { workspace = true }
async-trait = { workspace = true }
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

const MIGRATE_BIN_TEMPLATE: &str = r#"//! Migration binary — use `ravel migrate` in your project root instead.
//! ```bash
//! ravel migrate           # run pending migrations
//! ravel migrate:rollback   # rollback last migration
//! ravel migrate:status     # show migration status
//! ```

#[tokio::main]
async fn main() {
    println!("💡 Use `ravel migrate` from your project root to manage migrations.");
    println!();
    println!("   ravel migrate           # up");
    println!("   ravel migrate:rollback   # down");
    println!("   ravel migrate:status     # show status");
    println!("   ravel migrate:fresh     # drop all + re-apply");
}
"#;

const SEED_BIN_TEMPLATE: &str = r#"//! Seeder binary — use `ravel db:seed` in your project root instead.
//! ```bash
//! ravel db:seed   # run all registered seeders
//! ```

#[tokio::main]
async fn main() {
    println!("💡 Use `ravel db:seed` from your project root to run seeders.");
    println!();
    println!("   Define seeders in database/seeders/ and register them here.");
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
        let mut g = Generator::new("/tmp/test");
        // Register a test template then render it
        g.tera
            .add_raw_template("test_tpl", "Hello {{ name }}, your file is {{ snake }}.rs")
            .unwrap();
        let output = g.render("test_tpl", "UserController").unwrap();
        assert!(output.contains("UserController"));
        assert!(output.contains("user_controller.rs"));
        assert!(!output.contains("{{ name }}"));
    }

    #[test]
    fn test_generate_controller() {
        let tmp = std::env::temp_dir().join("ravel_gen_test");
        let _ = std::fs::remove_dir_all(&tmp);

        let g = Generator::new(&tmp);
        g.scaffold_controller("UserController").unwrap();

        let content =
            std::fs::read_to_string(tmp.join("app/Http/Controllers/UserController.rs")).unwrap();
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

        let content =
            std::fs::read_to_string(tmp.join("app/Http/Middleware/AuthMiddleware.rs")).unwrap();
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
        assert!(content.contains("pub async fn run(db: &DatabaseConnection)"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_generate_provider() {
        let tmp = std::env::temp_dir().join("ravel_gen_test5");
        let _ = std::fs::remove_dir_all(&tmp);

        let g = Generator::new(&tmp);
        g.scaffold_provider("RouteServiceProvider").unwrap();

        let content =
            std::fs::read_to_string(tmp.join("app/Providers/RouteServiceProvider.rs")).unwrap();
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

        let content =
            std::fs::read_to_string(tmp.join("app/Http/Requests/LoginRequest.rs")).unwrap();
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
        g.scaffold_project("MyApp", false).unwrap();

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
        assert!(migrate_bin.contains("ravel migrate"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_scaffold_project_dev_mode() {
        let tmp = std::env::temp_dir().join("ravel_gen_project_dev");
        let _ = std::fs::remove_dir_all(&tmp);

        let g = Generator::new(&tmp);
        g.scaffold_project("MyApp", true).unwrap();

        assert!(tmp.join("Cargo.toml").exists());
        assert!(tmp.join("src/main.rs").exists());
        assert!(tmp.join(".ravel-dev").exists());

        // Dev mode Cargo.toml should use path dependencies
        let cargo = std::fs::read_to_string(tmp.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("path = \"../crates/ravel-core\""));
        assert!(cargo.contains("path = \"../crates/ravel-http\""));
        assert!(!cargo.contains("git = \"https://github.com"));

        // .ravel-dev should contain framework_root
        let marker = std::fs::read_to_string(tmp.join(".ravel-dev")).unwrap();
        assert!(marker.contains("framework_root"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
