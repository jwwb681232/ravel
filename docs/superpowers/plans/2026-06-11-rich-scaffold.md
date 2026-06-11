# Rich Scaffold Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the bare `ravel new` scaffold with 15 source files demonstrating 14 framework features.

**Architecture:** All changes in `crates/ravel-cli/src/generator.rs` — 14 new Tera template constants + 2 updated + scaffold_project wiring + tests.

**Tech Stack:** Rust 2024, Tera templates

---

### Task 1: Update Cargo.toml templates — add new dependencies

**Files:**
- Modify: `crates/ravel-cli/src/generator.rs` (CARGO_TOML_TEMPLATE, CARGO_TOML_DEV_TEMPLATE)

- [ ] **Step 1: Add ravel-support and ravel-macros to CARGO_TOML_TEMPLATE**

Find `CARGO_TOML_TEMPLATE` (around line 511). After the `ravel-db-seaorm` line, add the two new deps:

```rust
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
"#;
```

- [ ] **Step 2: Add same deps to CARGO_TOML_DEV_TEMPLATE**

Find `CARGO_TOML_DEV_TEMPLATE` (around line 532). Add the two deps using `workspace = true`:

```rust
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
"#;
```

- [ ] **Step 3: Commit**

```bash
git add crates/ravel-cli/src/generator.rs
git commit -m "feat(scaffold): add ravel-support + ravel-macros to Cargo.toml templates"
```

---

### Task 2: Add database.toml + main_rs + routes_web templates

**Files:**
- Modify: `crates/ravel-cli/src/generator.rs`

- [ ] **Step 1: Add DATABASE_TOML_TEMPLATE**

Add after the `ENV_TEMPLATE` constant:

```rust
const DATABASE_TOML_TEMPLATE: &str = r#"# Database configuration
# Supports: sqlite, postgres, mysql

[default]
driver = "sqlite"
database = "database.sqlite"
"#;
```

- [ ] **Step 2: Rewrite MAIN_RS_TEMPLATE**

Replace the existing `MAIN_RS_TEMPLATE`:

```rust
const MAIN_RS_TEMPLATE: &str = r#"use ravel_core::app::Application;
use ravel_facades::Route;
use ravel_http::server;

mod routes;
mod app;

use app::Providers::{AppServiceProvider, RouteServiceProvider};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _app = Application::new()
        .load_env(".")?
        .load_config("config")?
        .with_cache()
        .with_app_key("base64:YOUR_APP_KEY_HERE")?
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
```

- [ ] **Step 3: Add ROUTES_WEB_TEMPLATE**

```rust
const ROUTES_WEB_TEMPLATE: &str = r#"use ravel_facades::Route;
use ravel_http::middleware::log_requests;
use crate::app::Http::Controllers::{UserController, PostController};

pub fn register() {
    // Global middleware: log all requests
    Route::middleware(log_requests);

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
```

- [ ] **Step 4: Commit**

```bash
git add crates/ravel-cli/src/generator.rs
git commit -m "feat(scaffold): add database.toml, main.rs rewrite, routes/web.rs templates"
```

---

### Task 3: Add Provider templates

**Files:**
- Modify: `crates/ravel-cli/src/generator.rs`

- [ ] **Step 1: Add ROUTE_SERVICE_PROVIDER_TEMPLATE**

```rust
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
```

- [ ] **Step 2: Add APP_SERVICE_PROVIDER_TEMPLATE**

```rust
const APP_SERVICE_PROVIDER_TEMPLATE: &str = r#"use ravel_core::app::ServiceProvider;
use ravel_core::container::Container;
use ravel_facades::Queue;
use anyhow::Result;

pub struct AppServiceProvider;

impl ServiceProvider for AppServiceProvider {
    fn register(&self, _container: &Container) -> Result<()> {
        // Register Queue with in-memory driver
        Queue::memory();
        Ok(())
    }

    fn name(&self) -> &str {
        "AppServiceProvider"
    }
}
"#;
```

- [ ] **Step 3: Commit**

```bash
git add crates/ravel-cli/src/generator.rs
git commit -m "feat(scaffold): add ServiceProvider templates"
```

---

### Task 4: Add Model templates (User + Post)

**Files:**
- Modify: `crates/ravel-cli/src/generator.rs`

- [ ] **Step 1: Add USER_MODEL_TEMPLATE**

```rust
const USER_MODEL_TEMPLATE: &str = r#"use ravel_eloquent::Model;

/// User model — one User has many Posts.
#[derive(Model, Clone, Debug)]
#[model(table = "users", timestamps)]
pub struct User {
    #[model(id)]
    pub id: i32,

    #[model(string, 255)]
    pub name: String,

    #[model(string, 254, unique)]
    pub email: String,

    #[model(hidden)]
    pub password: String,

    /// User has many Posts (foreign key: posts.user_id)
    pub posts: HasMany<Post>,
}
"#;
```

- [ ] **Step 2: Add POST_MODEL_TEMPLATE**

```rust
const POST_MODEL_TEMPLATE: &str = r#"use ravel_eloquent::Model;

/// Post model — each Post belongs to a User.
#[derive(Model, Clone, Debug)]
#[model(table = "posts", timestamps)]
pub struct Post {
    #[model(id)]
    pub id: i32,

    #[model(string, 255)]
    pub title: String,

    #[model(text)]
    pub content: String,

    #[model(integer)]
    #[model(belongs_to, from = "user_id", to = "id")]
    pub user_id: i32,

    /// The User who wrote this post
    pub user: BelongsTo<User>,
}
"#;
```

- [ ] **Step 3: Commit**

```bash
git add crates/ravel-cli/src/generator.rs
git commit -m "feat(scaffold): add User + Post Model templates"
```

---

### Task 5: Add Migration + Seeder templates

**Files:**
- Modify: `crates/ravel-cli/src/generator.rs`

- [ ] **Step 1: Add MIGRATION_USERS_TEMPLATE**

```rust
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
```

- [ ] **Step 2: Add MIGRATION_POSTS_TEMPLATE**

```rust
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
```

- [ ] **Step 3: Update MIGRATOR_TEMPLATE to include the two migrations**

Replace the existing `MIGRATOR_TEMPLATE`:

```rust
const MIGRATOR_TEMPLATE: &str = r#"use sea_orm_migration::prelude::*;

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
```

- [ ] **Step 4: Add USER_SEEDER_TEMPLATE**

```rust
const USER_SEEDER_TEMPLATE: &str = r#"use anyhow::Result;
use sea_orm::DatabaseConnection;

/// Seeder: create sample users
pub async fn run(db: &DatabaseConnection) -> Result<()> {
    tracing::info!("UserSeeder: seeding started");

    // Example: insert sample data with raw SQL
    db.execute_unprepared(
        "INSERT INTO users (name, email, password, created_at, updated_at) \
         VALUES ('Alice', 'alice@example.com', 'hashed_password', datetime('now'), datetime('now'))"
    ).await?;

    tracing::info!("UserSeeder: seeding complete");
    Ok(())
}
"#;
```

- [ ] **Step 5: Commit**

```bash
git add crates/ravel-cli/src/generator.rs
git commit -m "feat(scaffold): add Migration + Seeder templates"
```

---

### Task 6: Add Controller + FormRequest + Job templates

**Files:**
- Modify: `crates/ravel-cli/src/generator.rs`

- [ ] **Step 1: Add USER_CONTROLLER_TEMPLATE**

```rust
const USER_CONTROLLER_TEMPLATE: &str = r#"use ravel_http::controller::Controller;
use ravel_core::container::Container;
use ravel_facades::{Auth, Session, Config, Log, response, redirect};
use ravel_http::error::RavelError;
use axum::response::IntoResponse;
use ravel_http::session::Session as SessionExt;

pub struct UserController;

impl Controller for UserController {
    fn boot(_container: &Container) -> Self { Self }
}

impl UserController {
    /// GET /users — show usage of Config and Log facades
    pub async fn index() -> impl IntoResponse {
        let app_name: String = Config::get_or("app.name", "{{name}}");
        Log::info!("User list requested in {app_name}");
        response()
            .json(serde_json::json!({"users": ["alice", "bob"]}))
            .unwrap()
    }

    /// GET /users/{id} — requires login; shows Auth facade + RavelError
    pub async fn show(session: SessionExt, id: u32) -> Result<impl IntoResponse, RavelError> {
        if Auth::guest(&session) {
            return Ok(redirect("/login"));
        }
        let user_id: Option<i32> = Auth::id(&session);
        Log::info!("User {id} viewed by {user_id:?}");
        Ok(response()
            .json(serde_json::json!({"id": id, "name": "Alice"}))
            .unwrap())
    }

    /// POST /login — demonstrate Auth::login + Session::flash
    pub async fn login(mut session: SessionExt) -> impl IntoResponse {
        Auth::login(&mut session, 1);
        Session::flash(&mut session, "status", "Welcome back!");
        redirect("/")
    }

    /// POST /logout
    pub async fn logout(mut session: SessionExt) -> impl IntoResponse {
        Auth::logout(&mut session);
        redirect("/")
    }
}
"#;
```

- [ ] **Step 2: Add POST_CONTROLLER_TEMPLATE**

```rust
const POST_CONTROLLER_TEMPLATE: &str = r#"use ravel_http::controller::Controller;
use ravel_core::container::Container;
use ravel_facades::{Queue, response, abort};
use ravel_http::error::RavelError;
use ravel_http::form_request::Validated;
use axum::response::IntoResponse;
use sea_orm::DatabaseConnection;

use crate::app::Http::Requests::CreatePostRequest;
use crate::app::Models::Post;
use crate::app::Jobs::SendWelcomeEmail;

pub struct PostController;

impl Controller for PostController {
    fn boot(_container: &Container) -> Self { Self }
}

impl PostController {
    /// POST /posts — FormRequest validation, Model::create, Queue::dispatch
    pub async fn store(
        Validated(req): Validated<CreatePostRequest>,
        db: DatabaseConnection,
    ) -> Result<impl IntoResponse, RavelError> {
        // Create the post in the database
        let post = Post::create(serde_json::to_value(&req).map_err(|e| abort(500, e.to_string()))?, &db)
            .await
            .map_err(|e| abort(500, e.to_string()))?;

        // Dispatch a background job
        Queue::dispatch(SendWelcomeEmail { user_id: req.user_id })?;

        // Return the created post (hidden fields excluded)
        Ok(response()
            .status(201)
            .json(post.to_public_json())
            .unwrap())
    }
}
"#;
```

- [ ] **Step 3: Add CREATE_POST_REQUEST_TEMPLATE**

```rust
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
                Rule::Exists { table: "users", column: "id", ignore_id: None },
            ]),
        ]
    }
}
"#;
```

- [ ] **Step 4: Add SEND_WELCOME_JOB_TEMPLATE**

```rust
const SEND_WELCOME_JOB_TEMPLATE: &str = r#"use ravel_facades::Log;
use ravel_macros::Job;
use serde::{Serialize, Deserialize};

/// Background job: send a welcome email after a post is created.
#[derive(Serialize, Deserialize, Job)]
#[job(name = "send_welcome_email")]
pub struct SendWelcomeEmail {
    pub user_id: i32,
}

impl SendWelcomeEmail {
    pub async fn execute(&self) -> anyhow::Result<()> {
        Log::info!("Welcome email sent to user {}", self.user_id);
        Ok(())
    }
}
"#;
```

- [ ] **Step 5: Commit**

```bash
git add crates/ravel-cli/src/generator.rs
git commit -m "feat(scaffold): add Controller + FormRequest + Job templates"
```

---

### Task 7: Wire up scaffold_project + register all templates + add tests

**Files:**
- Modify: `crates/ravel-cli/src/generator.rs`

- [ ] **Step 1: Register all new templates in Generator::new**

In `Generator::new`, add `tera.add_raw_template` calls for the 14 new templates. Add these lines after the existing template registrations (after the `seed_bin` line):

```rust
tera.add_raw_template("database_toml", DATABASE_TOML_TEMPLATE).unwrap();
tera.add_raw_template("routes_web", ROUTES_WEB_TEMPLATE).unwrap();
tera.add_raw_template("user_model", USER_MODEL_TEMPLATE).unwrap();
tera.add_raw_template("post_model", POST_MODEL_TEMPLATE).unwrap();
tera.add_raw_template("user_controller", USER_CONTROLLER_TEMPLATE).unwrap();
tera.add_raw_template("post_controller", POST_CONTROLLER_TEMPLATE).unwrap();
tera.add_raw_template("create_post_request", CREATE_POST_REQUEST_TEMPLATE).unwrap();
tera.add_raw_template("send_welcome_job", SEND_WELCOME_JOB_TEMPLATE).unwrap();
tera.add_raw_template("app_service_provider", APP_SERVICE_PROVIDER_TEMPLATE).unwrap();
tera.add_raw_template("route_service_provider", ROUTE_SERVICE_PROVIDER_TEMPLATE).unwrap();
tera.add_raw_template("migration_users", MIGRATION_USERS_TEMPLATE).unwrap();
tera.add_raw_template("migration_posts", MIGRATION_POSTS_TEMPLATE).unwrap();
tera.add_raw_template("user_seeder", USER_SEEDER_TEMPLATE).unwrap();
// Note: no need to re-register "main_rs" — MAIN_RS_TEMPLATE was already replaced
// in Task 2 and the existing registration line points to the new constant.

- [ ] **Step 2: Update scaffold_project to generate all new files**

Replace the `scaffold_project` method body. After the `Cargo.toml` and `src/main.rs` writes, and after the dev marker block, add all new file generation:

```rust
// config/database.toml
if !self.exists("config/database.toml") {
    self.overwrite_file("config/database.toml", DATABASE_TOML_TEMPLATE)?;
}

// routes/web.rs
self.overwrite_file("routes/web.rs", &self.render("routes_web", project_name)?)?;

// app/Models/User.rs + Post.rs
self.create_file("app/Models/User.rs", &self.render("user_model", project_name)?)?;
self.create_file("app/Models/Post.rs", &self.render("post_model", project_name)?)?;

// app/Http/Controllers/UserController.rs + PostController.rs
self.create_file("app/Http/Controllers/UserController.rs", &self.render("user_controller", project_name)?)?;
self.create_file("app/Http/Controllers/PostController.rs", &self.render("post_controller", project_name)?)?;

// app/Http/Requests/CreatePostRequest.rs
self.create_file("app/Http/Requests/CreatePostRequest.rs", CREATE_POST_REQUEST_TEMPLATE)?;

// app/Jobs/SendWelcomeEmail.rs
self.create_file("app/Jobs/SendWelcomeEmail.rs", SEND_WELCOME_JOB_TEMPLATE)?;

// app/Providers/
self.create_file("app/Providers/AppServiceProvider.rs", APP_SERVICE_PROVIDER_TEMPLATE)?;
self.create_file("app/Providers/RouteServiceProvider.rs", ROUTE_SERVICE_PROVIDER_TEMPLATE)?;

// database/migrations/ (fixed names matching mod.rs)
self.create_file(
    "database/migrations/m0001_create_users_table.rs",
    MIGRATION_USERS_TEMPLATE,
)?;
self.create_file(
    "database/migrations/m0002_create_posts_table.rs",
    MIGRATION_POSTS_TEMPLATE,
)?;

// database/seeders/
self.create_file("database/seeders/UserSeeder.rs", USER_SEEDER_TEMPLATE)?;
```

Note: templates that contain `{{name}}` (like `routes_web`, `user_model`) use `self.render()`. Static templates (no Tera vars) use the constant directly.

- [ ] **Step 3: Update existing test test_scaffold_project**

In the `test_scaffold_project` test, add assertions for the new files. After the existing assertions, add:

```rust
// New scaffold files
assert!(tmp.join("config/database.toml").exists());
assert!(tmp.join("routes/web.rs").exists());
assert!(tmp.join("app/Models/User.rs").exists());
assert!(tmp.join("app/Models/Post.rs").exists());
assert!(tmp.join("app/Http/Controllers/UserController.rs").exists());
assert!(tmp.join("app/Http/Controllers/PostController.rs").exists());
assert!(tmp.join("app/Http/Requests/CreatePostRequest.rs").exists());
assert!(tmp.join("app/Jobs/SendWelcomeEmail.rs").exists());
assert!(tmp.join("app/Providers/AppServiceProvider.rs").exists());
assert!(tmp.join("app/Providers/RouteServiceProvider.rs").exists());
assert!(tmp.join("database/migrations/mod.rs").exists());

// Verify routes/web.rs content
let routes = std::fs::read_to_string(tmp.join("routes/web.rs")).unwrap();
assert!(routes.contains("Route::get"));
assert!(routes.contains("Route::group"));
assert!(routes.contains("log_requests"));

// Verify models
let user_model = std::fs::read_to_string(tmp.join("app/Models/User.rs")).unwrap();
assert!(user_model.contains("#[model(table = \"users\")"));
assert!(user_model.contains("HasMany<Post>"));

let post_model = std::fs::read_to_string(tmp.join("app/Models/Post.rs")).unwrap();
assert!(post_model.contains("BelongsTo<User>"));
```

- [ ] **Step 4: Run generator tests**

```bash
cargo test -p ravel-cli -- generator::tests
```

Expected: all tests pass (existing + new assertions).

- [ ] **Step 5: Build and verify no warnings**

```bash
cargo build -p ravel-cli
```

Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/ravel-cli/src/generator.rs
git commit -m "feat(scaffold): wire up rich scaffold with tests"
```

---

### Task 8: End-to-end verification

**Files:**
- None (verification only)

- [ ] **Step 1: Create a test project and verify all files**

```bash
cargo run -p ravel-cli -- new test-scaffold --dev
```

- [ ] **Step 2: Check file structure**

```bash
ls test-scaffold/config/database.toml
ls test-scaffold/routes/web.rs
ls test-scaffold/app/Models/User.rs
ls test-scaffold/app/Models/Post.rs
ls test-scaffold/app/Http/Controllers/UserController.rs
ls test-scaffold/app/Http/Controllers/PostController.rs
ls test-scaffold/app/Http/Requests/CreatePostRequest.rs
ls test-scaffold/app/Jobs/SendWelcomeEmail.rs
ls test-scaffold/app/Providers/AppServiceProvider.rs
ls test-scaffold/app/Providers/RouteServiceProvider.rs
ls test-scaffold/database/migrations/*.rs
ls test-scaffold/database/seeders/UserSeeder.rs
```

Expected: all files exist.

- [ ] **Step 3: Build the generated project**

```bash
cargo build -p test_scaffold
```

Expected: compiles successfully.

- [ ] **Step 4: Clean up and commit any fixes**

```bash
rm -rf test-scaffold
git add -A && git commit -m "chore: e2e verification of rich scaffold"
```
