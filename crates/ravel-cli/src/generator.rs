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
use base64::Engine;
use chrono::Utc;
use include_dir::{include_dir, Dir};
use rand::RngCore;
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use tera::{Context as TeraContext, Tera};

static TEMPLATES: Dir = include_dir!("$CARGO_MANIFEST_DIR/templates");

// ── Generator ─────────────────────────────────────────────────────

/// The scaffolding engine, rooted at a project directory.
pub struct Generator {
    root: PathBuf,
    tera: RefCell<Tera>,
}

impl Generator {
    /// Create a generator targeting `root` as the project root.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            tera: RefCell::new(Tera::default()),
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

    /// Render a template file by name with name-derived variables.
    ///
    /// `file_name` is the path relative to `templates/` (e.g. `"controller.rs"`).
    pub fn render(&self, file_name: &str, name: &str) -> Result<String> {
        let ctx = Self::make_context(name);
        let file = TEMPLATES
            .get_file(file_name)
            .with_context(|| format!("Template not found: '{file_name}'"))?;
        let src = file
            .contents_utf8()
            .with_context(|| format!("Template '{file_name}' is not valid UTF-8"))?;
        self.tera
            .borrow_mut()
            .render_str(src, &ctx)
            .with_context(|| format!("Failed to render template '{file_name}'"))
    }

    /// Render a template with an already-built Tera context.
    ///
    /// Use this when you need extra context variables beyond what
    /// `make_context()` provides (e.g. `app_key` for the env template).
    pub fn render_with_context(
        &self,
        file_name: &str,
        ctx: &TeraContext,
    ) -> Result<String> {
        let file = TEMPLATES
            .get_file(file_name)
            .with_context(|| format!("Template not found: '{file_name}'"))?;
        let src = file
            .contents_utf8()
            .with_context(|| format!("Template '{file_name}' is not valid UTF-8"))?;
        self.tera
            .borrow_mut()
            .render_str(src, ctx)
            .with_context(|| format!("Failed to render template '{file_name}'"))
    }

    // ── Built-in scaffolds ──────────────────────────────────────

    /// Generate a Controller file.
    pub fn scaffold_controller(&self, name: &str) -> Result<()> {
        let content = self.render("controller", name)?;
        let snake = to_snake(name);
        self.create_file(&format!("app/Http/Controllers/{snake}.rs"), &content)
    }

    /// Generate a Middleware file.
    pub fn scaffold_middleware(&self, name: &str) -> Result<()> {
        let content = self.render("middleware", name)?;
        let snake = to_snake(name);
        self.create_file(&format!("app/Http/Middleware/{snake}.rs"), &content)
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
        let snake = to_snake(name);
        self.create_file(&format!("database/seeders/{snake}.rs"), &content)
    }

    /// Generate a ServiceProvider file.
    pub fn scaffold_provider(&self, name: &str) -> Result<()> {
        let content = self.render("provider", name)?;
        let snake = to_snake(name);
        self.create_file(&format!("app/Providers/{snake}.rs"), &content)
    }

    /// Generate a FormRequest file.
    pub fn scaffold_request(&self, name: &str) -> Result<()> {
        let content = self.render("request", name)?;
        let snake = to_snake(name);
        self.create_file(&format!("app/Http/Requests/{snake}.rs"), &content)
    }

    /// Generate a Model file (SeaORM entity).
    pub fn scaffold_model(&self, name: &str) -> Result<()> {
        let content = self.render("model", name)?;
        let snake = to_snake(name);
        self.create_file(&format!("app/Models/{snake}.rs"), &content)
    }

    /// Generate a Job file.
    pub fn scaffold_job(&self, name: &str) -> Result<()> {
        let content = self.render("job", name)?;
        let snake = to_snake(name);
        self.create_file(&format!("app/Jobs/{snake}.rs"), &content)
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

        // Generate a random application key for this project
        let app_key = generate_app_key();

        // Build a Tera context that includes the app key
        let mut env_ctx = Self::make_context(project_name);
        env_ctx.insert("app_key", &app_key);
        let env_content = self
            .render_with_context("env", &env_ctx)
            .context("Failed to render env template")?;

        // Default .env
        if !self.exists(".env") {
            self.overwrite_file(".env", &env_content)?;
        }

        // .env.example (always overwrite to keep in sync)
        self.overwrite_file(".env.example", &env_content)?;

        // config/database.toml
        if !self.exists("config/database.toml") {
            self.overwrite_file("config/database.toml", DATABASE_TOML_TEMPLATE)?;
        }

        // Module declarations
        self.overwrite_file("routes/mod.rs", "pub mod web;\n")?;
        self.overwrite_file("app/mod.rs", "pub mod Http;\npub mod Models;\npub mod Jobs;\npub mod Providers;\n")?;
        self.overwrite_file("app/Http/mod.rs", "pub mod Controllers;\npub mod Requests;\n")?;
        self.overwrite_file("app/Models/mod.rs", "pub mod user;\npub mod post;\n")?;
        self.overwrite_file("app/Jobs/mod.rs", "pub mod send_welcome_email;\n")?;
        self.overwrite_file("app/Providers/mod.rs", "pub mod app_service_provider;\npub mod route_service_provider;\n")?;
        self.overwrite_file("app/Http/Requests/mod.rs", "pub mod create_post_request;\n")?;
        self.overwrite_file("app/Http/Controllers/mod.rs", "pub mod user_controller;\npub mod post_controller;\n")?;

        // routes/web.rs
        self.overwrite_file("routes/web.rs", &self.render("routes_web", project_name)?)?;

        // app/Models/
        self.create_file("app/Models/user.rs", &self.render("user_model", project_name)?)?;
        self.create_file("app/Models/post.rs", &self.render("post_model", project_name)?)?;

        // app/Http/Controllers/
        self.create_file("app/Http/Controllers/user_controller.rs", &self.render("user_controller", project_name)?)?;
        self.create_file("app/Http/Controllers/post_controller.rs", &self.render("post_controller", project_name)?)?;

        // app/Http/Requests/
        self.create_file("app/Http/Requests/create_post_request.rs", CREATE_POST_REQUEST_TEMPLATE)?;

        // app/Jobs/
        self.create_file("app/Jobs/send_welcome_email.rs", SEND_WELCOME_JOB_TEMPLATE)?;

        // app/Providers/
        self.create_file("app/Providers/app_service_provider.rs", APP_SERVICE_PROVIDER_TEMPLATE)?;
        self.create_file("app/Providers/route_service_provider.rs", ROUTE_SERVICE_PROVIDER_TEMPLATE)?;

        // database/migrations/
        self.create_file("database/migrations/m0001_create_users_table.rs", MIGRATION_USERS_TEMPLATE)?;
        self.create_file("database/migrations/m0002_create_posts_table.rs", MIGRATION_POSTS_TEMPLATE)?;

        // database/seeders/
        self.create_file("database/seeders/UserSeeder.rs", USER_SEEDER_TEMPLATE)?;

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

/// Generate a random 32-byte base64-encoded application key.
fn generate_app_key() -> String {
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    base64::engine::general_purpose::STANDARD.encode(&key)
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
ravel-support  = { git = "https://github.com/jwwb681232/ravel" }
ravel-macros   = { git = "https://github.com/jwwb681232/ravel" }
sea-orm             = { version = "2.0.0-rc.40", features = ["sqlx-sqlite", "runtime-tokio-rustls"] }
sea-orm-migration   = { version = "2.0.0-rc.40" }
axum   = { version = "0.8", features = ["multipart"] }
tokio  = { version = "1", features = ["full"] }
serde  = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow  = "1"
async-trait = "0.1"
chrono  = { version = "0.4", features = ["serde"] }
tracing = "0.1"
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
ravel-support  = { path = "../crates/ravel-support" }
ravel-macros   = { path = "../crates/ravel-macros" }
sea-orm             = { workspace = true, features = ["sqlx-sqlite", "runtime-tokio-rustls"] }
sea-orm-migration   = { workspace = true }
axum   = { workspace = true, features = ["multipart"] }
tokio  = { workspace = true, features = ["full"] }
serde  = { workspace = true }
serde_json = { workspace = true }
anyhow  = { workspace = true }
async-trait = { workspace = true }
chrono  = { workspace = true, features = ["serde"] }
tracing = { workspace = true }
"#;

const MAIN_RS_TEMPLATE: &str = r#"use ravel_core::app::Application;
use ravel_facades::Route;
use ravel_http::server;

#[path = "../app/mod.rs"]
mod app;
#[path = "../routes/mod.rs"]
mod routes;

use app::Providers::app_service_provider::AppServiceProvider;
use app::Providers::route_service_provider::RouteServiceProvider;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _app = Application::new()
        .load_env(env!("CARGO_MANIFEST_DIR"))?
        .load_config("config")?
        .with_cache()
        .with_app_key_from_env()?
        .register_provider(AppServiceProvider)
        .register_provider(RouteServiceProvider)
        .boot()?;

    let router = Route::build();

    let host = "127.0.0.1:3000";
    println!("{{name}} running at http://{host}");
    server::serve(router, host).await?;

    Ok(())
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
APP_KEY=base64:{{app_key}}
"#;

const DATABASE_TOML_TEMPLATE: &str = r#"# Database configuration
# Supports: sqlite, postgres, mysql

[default]
driver = "sqlite"
database = "database.sqlite"
"#;

const ROUTES_WEB_TEMPLATE: &str = r#"use ravel_facades::Route;
use crate::app::Http::Controllers::user_controller::UserController;
use crate::app::Http::Controllers::post_controller::PostController;

pub fn register() {
    // Home
    Route::get("/", || async { "Hello, {{name}}! 🚀" });

    // Auth (public)
    Route::post("/login", UserController::login);
    Route::post("/logout", UserController::logout);

    // Protected API group
    Route::group("/api", || {
        Route::get("/users", UserController::index);
        Route::get("/users/{id}", UserController::show);
        Route::post("/posts", PostController::store);
    });
}
"#;

const USER_MODEL_TEMPLATE: &str = r#"use ravel_eloquent::Model;
	use serde::{Serialize, Deserialize};

	#[derive(Model, Clone, Debug, Serialize, Deserialize)]
	#[model(table = "users", timestamps)]
	struct User {
	    #[model(id)]
	    id: i32,

	    #[model(string, 255)]
	    name: String,

	    #[model(string, 254, unique)]
	    email: String,

	    #[model(hidden)]
	    password: String,

	    created_at: chrono::NaiveDateTime,
	    updated_at: chrono::NaiveDateTime,
	}
	"#;

const POST_MODEL_TEMPLATE: &str = r#"use ravel_eloquent::Model;
	use serde::{Serialize, Deserialize};

	#[derive(Model, Clone, Debug, Serialize, Deserialize)]
	#[model(table = "posts", timestamps)]
	struct Post {
	    #[model(id)]
	    id: i32,

	    #[model(string, 255)]
	    title: String,

	    #[model(text)]
	    content: String,

	    #[model(integer)]
	    user_id: i32,

	    created_at: chrono::NaiveDateTime,
	    updated_at: chrono::NaiveDateTime,
	}
	"#;

const USER_CONTROLLER_TEMPLATE: &str = r#"use ravel_http::controller::Controller;
	use ravel_core::container::Container;
	use ravel_facades::Config;
	use axum::response::IntoResponse;

	pub struct UserController;

	impl Controller for UserController {
	    fn boot(_container: &Container) -> Self { Self }
	}

	impl UserController {
	    pub async fn index() -> impl IntoResponse {
	        let name: String = Config::get_or("app.name", "{{name}}".to_string());
	        format!("Welcome to {name} -- User list")
	    }

	    pub async fn show(axum::extract::Path(id): axum::extract::Path<u32>) -> impl IntoResponse {
	        format!("User {id}")
	    }

	    pub async fn login() -> impl IntoResponse {
	        "Logged in (Auth placeholder)"
	    }

	    pub async fn logout() -> impl IntoResponse {
	        "Logged out"
	    }
	}
	"#;

const POST_CONTROLLER_TEMPLATE: &str = r#"use ravel_http::controller::Controller;
	use ravel_core::container::Container;
	use axum::response::IntoResponse;

	pub struct PostController;

	impl Controller for PostController {
	    fn boot(_container: &Container) -> Self { Self }
	}

	impl PostController {
	    pub async fn store(
	        axum::extract::Json(body): axum::extract::Json<serde_json::Value>,
	    ) -> impl IntoResponse {
	        format!("Post created: {body}")
	    }
	}
	"#;

const CREATE_POST_REQUEST_TEMPLATE: &str = r#"use ravel_http::form_request::FormRequest;
use ravel_http::validation::{FieldRule, Rule};
use serde::Deserialize;

/// Validation rules for creating a post.
#[derive(Debug, Deserialize)]
pub struct CreatePostRequest {
    pub title: String,
    pub content: String,
    pub user_id: i32,
}

impl FormRequest for CreatePostRequest {
    fn rules() -> Vec<FieldRule> {
        vec![
            FieldRule::new("title", vec![
                Rule::Required,
                Rule::Min(3),
                Rule::Max(200),
            ]),
            FieldRule::new("content", vec![Rule::Required]),
            FieldRule::new("user_id", vec![
                Rule::Required,
                Rule::Exists { table: "users", column: "id" },
            ]),
        ]
    }
}
"#;

const SEND_WELCOME_JOB_TEMPLATE: &str = r#"use ravel_macros::Job;
use serde::{Serialize, Deserialize};

/// Background job: send a welcome email after a post is created.
#[derive(Serialize, Deserialize, Job)]
#[job(name = "send_welcome_email")]
pub struct SendWelcomeEmail {
    pub user_id: i32,
}

impl SendWelcomeEmail {
    pub async fn execute(&self) -> anyhow::Result<()> {
        tracing::info!("Welcome email sent to user {}", self.user_id);
        Ok(())
    }
}
"#;

const APP_SERVICE_PROVIDER_TEMPLATE: &str = r#"use ravel_core::app::ServiceProvider;
	use ravel_core::container::Container;
	use anyhow::Result;

	pub struct AppServiceProvider;

	impl ServiceProvider for AppServiceProvider {
	    fn register(&self, _container: &Container) -> Result<()> {
	        // Register services here (Queue, Storage, etc.)
	        Ok(())
	    }

	    fn name(&self) -> &str {
	        "AppServiceProvider"
	    }
	}
	"#;

const ROUTE_SERVICE_PROVIDER_TEMPLATE: &str = r#"use ravel_core::app::ServiceProvider;
use ravel_core::container::Container;
use anyhow::Result;

pub struct RouteServiceProvider;

impl ServiceProvider for RouteServiceProvider {
    fn register(&self, _container: &Container) -> Result<()> {
        Ok(())
    }

    fn boot(&self, _container: &Container) -> Result<()> {
        crate::routes::web::register();
        Ok(())
    }

    fn name(&self) -> &str {
        "RouteServiceProvider"
    }
}
"#;

const MIGRATION_USERS_TEMPLATE: &str = r#"use sea_orm_migration::prelude::*;

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
                    .col(ColumnDef::new(Users::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Users::Name).string().not_null())
                    .col(ColumnDef::new(Users::Email).string().not_null().unique_key())
                    .col(ColumnDef::new(Users::Password).string().not_null())
                    .col(ColumnDef::new(Users::CreatedAt).timestamp().not_null()
                        .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)))
                    .col(ColumnDef::new(Users::UpdatedAt).timestamp().not_null()
                        .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Users::Table).to_owned()).await
    }
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
    Name,
    Email,
    Password,
    CreatedAt,
    UpdatedAt,
}
"#;

const MIGRATION_POSTS_TEMPLATE: &str = r#"use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Posts::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Posts::Id).integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Posts::UserId).integer().not_null())
                    .col(ColumnDef::new(Posts::Title).string().not_null())
                    .col(ColumnDef::new(Posts::Content).text().not_null())
                    .col(ColumnDef::new(Posts::CreatedAt).timestamp().not_null()
                        .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)))
                    .col(ColumnDef::new(Posts::UpdatedAt).timestamp().not_null()
                        .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_posts_user_id")
                            .from(Posts::Table, Posts::UserId)
                            .to(Users::Table, Users::Id)
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Posts::Table).to_owned()).await
    }
}

#[derive(Iden)]
enum Posts {
    Table,
    Id,
    UserId,
    Title,
    Content,
    CreatedAt,
    UpdatedAt,
}
"#;

const USER_SEEDER_TEMPLATE: &str = r#"use anyhow::Result;
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
"#;

const MIGRATOR_TEMPLATE: &str = r#"use sea_orm_migration::prelude::*;

mod m0001_create_users_table;
mod m0002_create_posts_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m0001_create_users_table::Migration),
            Box::new(m0002_create_posts_table::Migration),
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
            std::fs::read_to_string(tmp.join("app/Http/Controllers/user_controller.rs")).unwrap();
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
            std::fs::read_to_string(tmp.join("app/Http/Middleware/auth_middleware.rs")).unwrap();
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

        let content = std::fs::read_to_string(tmp.join("database/seeders/user_seeder.rs")).unwrap();
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
            std::fs::read_to_string(tmp.join("app/Providers/route_service_provider.rs")).unwrap();
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
            std::fs::read_to_string(tmp.join("app/Http/Requests/login_request.rs")).unwrap();
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
        assert!(tmp.join("config/database.toml").exists());
        assert!(tmp.join("routes/web.rs").exists());
        assert!(tmp.join("app/Http/Controllers").is_dir());
        assert!(tmp.join("app/Models/user.rs").exists());
        assert!(tmp.join("app/Models/post.rs").exists());
        assert!(tmp.join("app/Http/Controllers/user_controller.rs").exists());
        assert!(tmp.join("app/Http/Controllers/post_controller.rs").exists());
        assert!(tmp.join("app/Http/Requests/create_post_request.rs").exists());
        assert!(tmp.join("app/Jobs/send_welcome_email.rs").exists());
        assert!(tmp.join("app/Providers/app_service_provider.rs").exists());
        assert!(tmp.join("app/Providers/route_service_provider.rs").exists());
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
