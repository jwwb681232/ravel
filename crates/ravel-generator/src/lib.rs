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

const MIGRATION_TEMPLATE: &str = r#"use anyhow::Result;

/// Migration: {{name}}
///
/// Timestamp: {{timestamp}}

pub fn up() -> Result<()> {
    // TODO: apply the migration
    Ok(())
}

pub fn down() -> Result<()> {
    // TODO: rollback the migration
    Ok(())
}
"#;

const SEEDER_TEMPLATE: &str = r#"use anyhow::Result;

/// Seeder: {{name}}

pub fn run() -> Result<()> {
    // TODO: insert seed data
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

const REQUEST_TEMPLATE: &str = r#"use serde::Deserialize;

/// Form request: {{name}}
#[derive(Debug, Deserialize)]
pub struct {{name}} {
    // TODO: define validation fields
}

impl {{name}} {
    /// Validate the request data.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // TODO: add validation rules

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
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
        assert!(content.contains("fn up()"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_generate_seeder() {
        let tmp = std::env::temp_dir().join("ravel_gen_test4");
        let _ = std::fs::remove_dir_all(&tmp);

        let g = Generator::new(&tmp);
        g.scaffold_seeder("UserSeeder").unwrap();

        let content = std::fs::read_to_string(tmp.join("database/seeders/UserSeeder.rs")).unwrap();
        assert!(content.contains("pub fn run()"));

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
        assert!(content.contains("fn validate"));

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

        let cargo = std::fs::read_to_string(tmp.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("my_app"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
