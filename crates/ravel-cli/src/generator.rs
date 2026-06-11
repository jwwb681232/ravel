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
        let content = self.render("controller.rs", name)?;
        let snake = to_snake(name);
        self.create_file(&format!("app/http/controllers/{snake}.rs"), &content)
    }

    /// Generate a Middleware file.
    pub fn scaffold_middleware(&self, name: &str) -> Result<()> {
        let content = self.render("middleware.rs", name)?;
        let snake = to_snake(name);
        self.create_file(&format!("app/http/middleware/{snake}.rs"), &content)
    }

    /// Generate a Migration file (timestamped).
    pub fn scaffold_migration(&self, name: &str) -> Result<()> {
        let content = self.render("migration.rs", name)?;
        let timestamp = Utc::now().format("%Y_%m_%d_%H%M%S");
        let snake = to_snake(name);
        self.create_file(
            &format!("database/migrations/{}_{}.rs", timestamp, snake),
            &content,
        )
    }

    /// Generate a Seeder file.
    pub fn scaffold_seeder(&self, name: &str) -> Result<()> {
        let content = self.render("seeder.rs", name)?;
        let snake = to_snake(name);
        self.create_file(&format!("database/seeders/{snake}.rs"), &content)
    }

    /// Generate a ServiceProvider file.
    pub fn scaffold_provider(&self, name: &str) -> Result<()> {
        let content = self.render("provider.rs", name)?;
        let snake = to_snake(name);
        self.create_file(&format!("app/providers/{snake}.rs"), &content)
    }

    /// Generate a FormRequest file.
    pub fn scaffold_request(&self, name: &str) -> Result<()> {
        let content = self.render("request.rs", name)?;
        let snake = to_snake(name);
        self.create_file(&format!("app/http/requests/{snake}.rs"), &content)
    }

    /// Generate a Model file (SeaORM entity).
    pub fn scaffold_model(&self, name: &str) -> Result<()> {
        let content = self.render("model.rs", name)?;
        let snake = to_snake(name);
        self.create_file(&format!("app/models/{snake}.rs"), &content)
    }

    /// Generate a Job file.
    pub fn scaffold_job(&self, name: &str) -> Result<()> {
        let content = self.render("job.rs", name)?;
        let snake = to_snake(name);
        self.create_file(&format!("app/jobs/{snake}.rs"), &content)
    }

    /// Read a static template file (no variable substitution).
    fn raw_template(file_name: &str) -> Result<&'static str> {
        TEMPLATES
            .get_file(file_name)
            .and_then(|f| f.contents_utf8())
            .with_context(|| format!("Template not found or not UTF-8: '{file_name}'"))
    }

    /// Scaffold the initial project skeleton (used by `ravel new`).
    ///
    /// When `dev` is true, Cargo.toml uses `path` dependencies
    /// pointing to the local framework crates, and a `.ravel-dev`
    /// marker file is written.
    pub fn scaffold_project(&self, project_name: &str, dev: bool) -> Result<()> {
        let dirs = [
            "app/http/controllers",
            "app/http/middleware",
            "app/http/requests",
            "app/models",
            "app/jobs",
            "app/services",
            "app/providers",
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
        let cargo_file = if dev {
            "cargo-dev.toml"
        } else {
            "cargo-release.toml"
        };
        self.overwrite_file(
            "Cargo.toml",
            &self.render(cargo_file, project_name)?,
        )?;

        // src/main.rs
        self.overwrite_file("src/main.rs", &self.render("main.rs", project_name)?)?;

        // Default config file
        self.overwrite_file("config/app.toml", &self.render("app.toml", project_name)?)?;

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
            self.overwrite_file("config/database.toml", Self::raw_template("database.toml")?)?;
        }

        // Module declarations
        self.overwrite_file("routes/mod.rs", "pub mod web;\n")?;
        self.overwrite_file("app/mod.rs", "pub mod http;\npub mod models;\npub mod jobs;\npub mod providers;\n")?;
        self.overwrite_file("app/http/mod.rs", "pub mod controllers;\npub mod middleware;\npub mod requests;\n")?;
        self.overwrite_file("app/models/mod.rs", "pub mod user;\npub mod post;\n")?;
        self.overwrite_file("app/jobs/mod.rs", "pub mod send_welcome_email;\n")?;
        self.overwrite_file("app/providers/mod.rs", "pub mod app_service_provider;\npub mod route_service_provider;\n")?;
        self.overwrite_file("app/http/requests/mod.rs", "pub mod create_post_request;\n")?;
        self.overwrite_file("app/http/controllers/mod.rs", "pub mod user_controller;\npub mod post_controller;\n")?;

        // routes/web.rs
        self.overwrite_file("routes/web.rs", &self.render("routes-web.rs", project_name)?)?;

        // app/models/
        self.create_file("app/models/user.rs", &self.render("user-model.rs", project_name)?)?;
        self.create_file("app/models/post.rs", &self.render("post-model.rs", project_name)?)?;

        // app/http/controllers/
        self.create_file("app/http/controllers/user_controller.rs", &self.render("user-controller.rs", project_name)?)?;
        self.create_file("app/http/controllers/post_controller.rs", &self.render("post-controller.rs", project_name)?)?;

        // app/http/requests/
        self.create_file("app/http/requests/create_post_request.rs", Self::raw_template("create-post-request.rs")?)?;

        // app/jobs/
        self.create_file("app/jobs/send_welcome_email.rs", Self::raw_template("send-welcome-job.rs")?)?;

        // app/providers/
        self.create_file("app/providers/app_service_provider.rs", Self::raw_template("app-service-provider.rs")?)?;
        self.create_file("app/providers/route_service_provider.rs", Self::raw_template("route-service-provider.rs")?)?;

        // database/migrations/
        self.create_file("database/migrations/m0001_create_users_table.rs", Self::raw_template("migration-users.rs")?)?;
        self.create_file("database/migrations/m0002_create_posts_table.rs", Self::raw_template("migration-posts.rs")?)?;

        // database/seeders/
        self.create_file("database/seeders/user_seeder.rs", Self::raw_template("user-seeder.rs")?)?;

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
            self.overwrite_file("database/migrations/mod.rs", Self::raw_template("migrator.rs")?)?;
        }

        // Migrate binary (src/bin/migrate.rs)
        if !self.exists("src/bin/migrate.rs") {
            self.overwrite_file("src/bin/migrate.rs", Self::raw_template("migrate-bin.rs")?)?;
        }

        // Seed binary (src/bin/seed.rs)
        if !self.exists("src/bin/seed.rs") {
            self.overwrite_file("src/bin/seed.rs", Self::raw_template("seed-bin.rs")?)?;
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
        // Render an actual template from include_dir!
        let output = g.render("model.rs", "UserController").unwrap();
        assert!(output.contains("UserController"));
        assert!(output.contains("user_controllers"));
        assert!(!output.contains("{{ name }}"));
    }

    #[test]
    fn test_generate_controller() {
        let tmp = std::env::temp_dir().join("ravel_gen_test");
        let _ = std::fs::remove_dir_all(&tmp);

        let g = Generator::new(&tmp);
        g.scaffold_controller("UserController").unwrap();

        let content =
            std::fs::read_to_string(tmp.join("app/http/controllers/user_controller.rs")).unwrap();
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
            std::fs::read_to_string(tmp.join("app/http/middleware/auth_middleware.rs")).unwrap();
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
            std::fs::read_to_string(tmp.join("app/providers/route_service_provider.rs")).unwrap();
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
            std::fs::read_to_string(tmp.join("app/http/requests/login_request.rs")).unwrap();
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
        assert!(tmp.join("app/http/controllers").is_dir());
        assert!(tmp.join("app/models/user.rs").exists());
        assert!(tmp.join("app/models/post.rs").exists());
        assert!(tmp.join("app/http/controllers/user_controller.rs").exists());
        assert!(tmp.join("app/http/controllers/post_controller.rs").exists());
        assert!(tmp.join("app/http/requests/create_post_request.rs").exists());
        assert!(tmp.join("app/jobs/send_welcome_email.rs").exists());
        assert!(tmp.join("app/providers/app_service_provider.rs").exists());
        assert!(tmp.join("app/providers/route_service_provider.rs").exists());
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
