# Quick Tour: Building a Blog

A complete blog with user registration, login, CRUD posts, and logout. All code uses the facade API and `#[derive(Model)]` Eloquent ORM.

---

## 1. Project Setup

```bash
ravel new MyBlog && cd MyBlog
```

```
MyBlog/
  Cargo.toml              # dependencies
  config/app.toml         # database, server config
  bootstrap/app.rs        # Application boot + RouteServiceProvider
  src/main.rs             # entry point
  templates/              # Tera templates
```

Add to `Cargo.toml`:

```toml
[dependencies]
ravel-core = { path = "../crates/ravel-core" }
ravel-facades = { path = "../crates/ravel-facades" }
ravel-http = { path = "../crates/ravel-http" }
ravel-eloquent = { path = "../crates/ravel-eloquent" }
ravel-db-seaorm = { path = "../crates/ravel-db-seaorm" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
sea-orm = { version = "2", features = ["sqlx-sqlite", "runtime-tokio-rustls", "macros"] }
```

---

## 2. Database Setup

`config/app.toml`:

```toml
[database]
default = "sqlite"
[database.connections.sqlite]
driver = "sqlite"
database = "database/blog.sqlite"
[server]
host = "127.0.0.1"
port = 3000
```

Create a `DatabaseServiceProvider` to manage the connection:

```rust
// bootstrap/providers/database.rs
use ravel_core::app::ServiceProvider;
use ravel_core::container::Container;
use ravel_db_seaorm::connection::ConnectionManager;
use sea_orm::DatabaseConnection;
use std::sync::OnceLock;

static DB: OnceLock<DatabaseConnection> = OnceLock::new();

pub fn db() -> &'static DatabaseConnection {
    DB.get().expect("Database not initialized — call DatabaseServiceProvider first")
}

pub struct DatabaseServiceProvider;

impl ServiceProvider for DatabaseServiceProvider {
    fn register(&self, _container: &Container) -> anyhow::Result<()> {
        let manager = ConnectionManager::from_config("config")?;
        let conn = smol::block_on(manager.connect("default"))?;
        DB.set(conn).ok();
        Ok(())
    }

    fn name(&self) -> &str { "DatabaseServiceProvider" }
}
```

Migrations — use `ravel make:migration` to scaffold:

```rust
// create_users_table
use ravel_db_core::schema::Schema;

fn up() {
    Schema::create("users", |t| {
        t.id();
        t.string("name", 255);
        t.string("email", 255).unique();
        t.string("password", 255);
        t.timestamps();
    });
}

// create_posts_table
fn up() {
    Schema::create("posts", |t| {
        t.id();
        t.string("title", 255);
        t.text("body");
        t.integer("user_id");
        t.timestamps();
    });
}
```

Run `ravel migrate`.

---

## 3. Models

Define Eloquent models with `#[derive(Model)]`. The `timestamps` container attribute
auto-adds `created_at` / `updated_at` fields (no need to declare them manually).

```rust
// src/models/user.rs
use ravel_eloquent::Model;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Model, Serialize, Deserialize)]
#[model(table = "users", timestamps)]
pub struct User {
    #[model(id)]
    pub id: i32,

    #[model(string, 255)]
    pub name: String,

    #[model(string, 255, unique)]
    pub email: String,

    #[model(hidden)]
    pub password: String,
}
```

```rust
// src/models/post.rs
use ravel_eloquent::Model;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Model, Serialize, Deserialize)]
#[model(table = "posts", timestamps)]
pub struct Post {
    #[model(id)]
    pub id: i32,

    #[model(string, 255)]
    pub title: String,

    #[model(text)]
    pub body: String,

    #[model(integer)]
    pub user_id: i32,
}

impl Post {
    pub fn owned_by(&self, uid: i32) -> bool {
        self.user_id == uid
    }
}
```

`#[derive(Model)]` generates:

**Types:**
- `UserColumn` / `PostColumn` enums (PascalCase variants, `as_str()`, `FromStr`, `ColumnTrait`)
- `UserPublic` / `PostPublic` structs (hidden fields excluded)
- `UserEntity` / `PostEntity` — SeaORM `EntityTrait` impl
- `UserActiveModel` / `PostActiveModel` — `ActiveValue<Value>` fields for insert/update
- `UserPrimaryKey` / `PostPrimaryKey` — `PrimaryKeyTrait` impl

**Traits:**
- `ModelMeta`, `ModelExt`, `ActiveModelExt`, `Fillable`, `Serializes`, `Replicates`
- `FromQueryResult`, `ModelTrait` (SeaORM compatibility)

**Methods:**
- `query()`, `query_with_trashed()`, `where_str()`, `find()`, `find_or_fail()`, `all()`, `all_with_trashed()`, `all_only_trashed()`, `create()`, `destroy()`
- `save()`, `insert()`, `update()`, `delete()`, `force_delete()`, `restore()`, `refresh()`, `replicate()`, `touch()`
- `set_name()`, `set_email()`, … per-field chainable setters
- `to_public()`, `to_json()`, `to_public_json()`, `fill()`

---

## 4. User Registration

### Form Request & Handler

```rust
use ravel_http::form_request::{FormRequest, Validated};
use ravel_http::validation::{FieldRule, Rule};
use ravel_http::error::RavelError;
use ravel_facades::{Hash, redirect, Session};
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::models::user::User;
use crate::providers::database::db;

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub name: String,
    pub email: String,
    pub password: String,
}

impl FormRequest for RegisterRequest {
    fn rules() -> Vec<FieldRule> {
        vec![
            FieldRule::new("name", vec![Rule::Required, Rule::Min(2)]),
            FieldRule::new("email", vec![Rule::Required, Rule::Email]),
            FieldRule::new("password", vec![Rule::Required, Rule::Min(6)]),
        ]
    }
}

pub async fn register(
    Validated(req): Validated<RegisterRequest>,
) -> Result<impl IntoResponse, RavelError> {
    let hashed = Hash::make(&req.password)
        .map_err(|e| RavelError::internal(format!("Hash failed: {e}")))?;

    // Eloquent: create from JSON
    User::create(
        serde_json::json!({"name": req.name, "email": req.email, "password": hashed}),
        db(),
    )
    .await
    .map_err(|e| RavelError::internal(e.to_string()))?;

    Session::flash("status", "Registration successful! Please log in.");
    Ok(redirect("/login"))
}
```

---

## 5. Login

```rust
use ravel_facades::Auth;
use crate::models::user::User;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

impl FormRequest for LoginRequest {
    fn rules() -> Vec<FieldRule> {
        vec![
            FieldRule::new("email", vec![Rule::Required, Rule::Email]),
            FieldRule::new("password", vec![Rule::Required]),
        ]
    }
}

pub async fn login(
    Validated(req): Validated<LoginRequest>,
) -> Result<impl IntoResponse, RavelError> {
    // Eloquent: query by email, get first match
    let user: Option<User> = User::query()
        .where_str("email", &req.email)
        .first(db())
        .await
        .map_err(|e| RavelError::internal(e.to_string()))?;

    let user = user.ok_or(RavelError::unauthorized("Invalid credentials"))?;

    if !Hash::check(&req.password, &user.password)
        .map_err(|e| RavelError::internal(e.to_string()))?
    {
        return Err(RavelError::unauthorized("Invalid credentials"));
    }

    Auth::login(&user.id);
    Session::flash("status", "Welcome back!");
    Ok(redirect("/posts"))
}
```

Flow: `Validated<T>` → query user → `Hash::check()` → `Auth::login()` → flash → redirect.

---

## 6. Posts CRUD

All handlers now use the type-safe Eloquent API rather than raw SQL strings.

### List all posts

```rust
use crate::models::post::Post;

pub async fn index() -> Result<impl IntoResponse, RavelError> {
    // Eloquent: type-safe query with ordering
    let posts = Post::query()
        .order_by_desc(PostColumn::CreatedAt)
        .get(db())
        .await
        .map_err(|e| RavelError::internal(e.to_string()))?;

    Ok(axum::Json(serde_json::json!({ "posts": posts })))
}
```

### Show a single post

```rust
use axum::extract::Path;

pub async fn show(Path(id): Path<i32>) -> Result<impl IntoResponse, RavelError> {
    // Eloquent: find by primary key, error if missing
    let post = Post::find_or_fail(db(), id)
        .await
        .map_err(|e| RavelError::not_found(e.to_string()))?;

    Ok(axum::Json(serde_json::json!({ "post": post })))
}
```

### Create form

```rust
pub async fn create() -> impl IntoResponse {
    if Auth::guest() {
        return redirect("/login");
    }
    "Create Post form" // render template in practice
}
```

### Store (insert)

```rust
#[derive(Debug, Deserialize)]
pub struct StorePostRequest {
    pub title: String,
    pub body: String,
}

impl FormRequest for StorePostRequest {
    fn rules() -> Vec<FieldRule> {
        vec![
            FieldRule::new("title", vec![Rule::Required, Rule::Min(3)]),
            FieldRule::new("body", vec![Rule::Required, Rule::Min(10)]),
        ]
    }
}

pub async fn store(
    Validated(req): Validated<StorePostRequest>,
) -> Result<impl IntoResponse, RavelError> {
    let uid: i32 = Auth::id().ok_or(RavelError::unauthorized("Not logged in"))?;

    // Eloquent: create from JSON
    Post::create(
        serde_json::json!({"title": req.title, "body": req.body, "user_id": uid}),
        db(),
    )
    .await
    .map_err(|e| RavelError::internal(e.to_string()))?;

    Session::flash("status", "Post created!");
    Ok(redirect("/posts"))
}
```

### Edit form

```rust
pub async fn edit(Path(id): Path<i32>) -> Result<impl IntoResponse, RavelError> {
    if Auth::guest() {
        return Ok(redirect("/login"));
    }

    let post = Post::find_or_fail(db(), id)
        .await
        .map_err(|e| RavelError::not_found(e.to_string()))?;

    if !post.owned_by(Auth::id::<i32>().unwrap_or(0)) {
        return Err(RavelError::forbidden("Not your post"));
    }

    // to_public() strips hidden fields — safe for API responses
    Ok(axum::Json(serde_json::json!({ "post": post.to_public() })))
}
```

### Update

```rust
pub async fn update(
    Path(id): Path<i32>,
    Validated(req): Validated<StorePostRequest>,
) -> Result<impl IntoResponse, RavelError> {
    let post = Post::find_or_fail(db(), id)
        .await
        .map_err(|e| RavelError::not_found(e.to_string()))?;

    if !post.owned_by(Auth::id::<i32>().unwrap_or(0)) {
        return Err(RavelError::forbidden("Not your post"));
    }

    // Eloquent: chain set_xxx() + save()
    post.set_title(req.title)
        .set_body(req.body)
        .save(db())
        .await
        .map_err(|e| RavelError::internal(e.to_string()))?;

    Session::flash("status", "Post updated!");
    Ok(redirect("/posts"))
}
```

Key change from v1: no more `post.update(json)` with raw SQL — the `save()` method auto-detects INSERT vs UPDATE based on whether `id` is zero.

### Delete

```rust
pub async fn destroy(Path(id): Path<i32>) -> Result<impl IntoResponse, RavelError> {
    let post = Post::find_or_fail(db(), id)
        .await
        .map_err(|e| RavelError::not_found(e.to_string()))?;

    if !post.owned_by(Auth::id::<i32>().unwrap_or(0)) {
        return Err(RavelError::forbidden("Not your post"));
    }

    // Eloquent: consumptive delete (post is consumed)
    // With soft_deletes enabled, this would SET deleted_at = NOW()
    post.delete(db())
        .await
        .map_err(|e| RavelError::internal(e.to_string()))?;

    Session::flash("status", "Post deleted!");
    Ok(redirect("/posts"))
}

// Alternative — delete without loading the instance first:
// Post::destroy(db(), id).await?;
```

---

## 7. Relationships

With v2, relationship methods are generated automatically by `#[derive(Model)]`.
Add relation fields to your models to enable lazy-loaded relationship queries.

```rust
// On the User model, add:
#[derive(Debug, Clone, Model, Serialize, Deserialize)]
#[model(table = "users", timestamps)]
pub struct User {
    #[model(id)]       pub id: i32,
    pub name: String,
    #[model(string, 254, unique)] pub email: String,
    #[model(hidden)]   pub password: String,

    // Each user has many posts
    #[model(has_many)]
    pub posts: ravel_eloquent::HasMany<Post>,
}

// On the Post model, add:
#[derive(Debug, Clone, Model, Serialize, Deserialize)]
#[model(table = "posts", timestamps)]
pub struct Post {
    #[model(id)]       pub id: i32,
    pub title: String,
    #[model(text)]     pub body: String,
    #[model(integer)]  pub user_id: i32,

    // Each post belongs to a user
    #[model(belongs_to, from = "user_id", to = "id")]
    pub author: ravel_eloquent::HasOne<User>,
}
```

### Lazy-loading related records

```rust
// ── Get all posts by a user ──
let user = User::find_or_fail(db(), 1).await?;

// user.posts() returns RelationQuery<Post> — supports filter, order, limit, paginate
let posts = user.posts()
    .where_eq(PostColumn::UserId, user.id)
    .order_by_desc(PostColumn::CreatedAt)
    .limit(10)
    .get(db())
    .await?;

// ── Get the author of a post ──
let post = Post::find_or_fail(db(), 5).await?;
let author = post.author().first(db()).await?;

// ── Count related records ──
let post_count = user.posts().count(db()).await?;
let has_posts = user.posts().exists(db()).await?;
```

### Eager-loading with `.with()`

```rust
// Preload related data in a single pass
let users = User::query()
    .where_str("active", true)
    .with("posts")
    .get(db()).await?;

// Or via static method
let users = User::all_with(db(), &["posts"]).await?;

for user in &users {
    println!("{} has {} posts", user.name, user.posts.len());
}
```

### Many-to-Many (BelongsToMany)

```rust
// Define with pivot table
#[derive(Model, ...)]
#[model(table = "users")]
struct User {
    #[model(id)] pub id: i32,
    #[model(has_many, via = "role_user")]
    pub roles: ravel_eloquent::BelongsToMany<Role>,
}

// Eager-load
let users = User::all_with(&db, &["roles"]).await?;

// Pivot operations
let user = User::find(&db, 1).await?;
user.attach("roles", &[1, 2], &db).await?;      // add roles
user.detach("roles", &[2], &db).await?;          // remove specific roles
user.sync("roles", &[1, 3], &db).await?;         // replace all roles
```

### Soft Deletes

```rust
#[derive(Model, ...)]
#[model(table = "posts", soft_deletes)]
struct Post {
    #[model(id)] pub id: i32,
    pub title: String,
}

// Normal queries filter out trashed records automatically
let active = Post::all(&db).await?;  // WHERE deleted_at IS NULL

// Include trashed
let all = Post::all_with_trashed(&db).await?;

// Only trashed
let trashed = Post::all_only_trashed(&db).await?;

// Soft delete
post.delete(&db).await?;  // UPDATE SET deleted_at = NOW()

// Hard delete
post.force_delete(&db).await?;  // DELETE FROM

// Restore
let restored = trashed.restore(&db).await?;  // SET deleted_at = NULL
```

---

## 8. Auth Guards

### Per-Handler

```rust
pub async fn dashboard() -> impl IntoResponse {
    if Auth::guest() {
        Session::flash("status", "Please log in.");
        return redirect("/login");
    }
    format!("Welcome, user {}!", Auth::id::<i32>().unwrap_or(0))
}
```

### AuthGuard Middleware

```rust
use ravel_http::auth::AuthGuard;

Route::group("/admin", || {
    Route::get("/dashboard", dashboard);
});
Route::middleware(AuthGuard::middleware()); // 401 if no session cookie
```

### Flash Messages

```rust
Session::flash("status", "Done!");                      // write (next request)
let msg: Option<String> = Session::flashed("status");  // read once
```

---

## 9. Logout

```rust
pub async fn logout() -> impl IntoResponse {
    Auth::logout();
    Session::flash("status", "Logged out.");
    redirect("/")
}
// Register: Route::post("/logout", handlers::auth::logout);
```

---

## 10. Full Router Setup

`bootstrap/app.rs`:

```rust
use ravel_core::app::{Application, ServiceProvider};
use ravel_core::container::Container;
use ravel_facades::Route;
use ravel_http::auth::AuthGuard;

use crate::providers::database::DatabaseServiceProvider;

pub struct RouteServiceProvider;

impl ServiceProvider for RouteServiceProvider {
    fn register(&self, _c: &Container) -> anyhow::Result<()> {
        Route::get("/", || async { redirect("/posts") });
        Route::get("/register", handlers::auth::register_form);
        Route::post("/register", handlers::auth::register);
        Route::get("/login", handlers::auth::login_form);
        Route::post("/login", handlers::auth::login);

        Route::group("/posts", || {
            Route::get("/", handlers::posts::index);
            Route::get("/create", handlers::posts::create);
            Route::post("/", handlers::posts::store);
            Route::get("/{id}", handlers::posts::show);
            Route::get("/{id}/edit", handlers::posts::edit);
            Route::put("/{id}", handlers::posts::update);
            Route::delete("/{id}", handlers::posts::destroy);
        });

        Route::middleware(AuthGuard::middleware());
        Route::post("/logout", handlers::auth::logout);
        Ok(())
    }

    fn name(&self) -> &str { "RouteServiceProvider" }
}

pub fn create_app() {
    Application::new()
        .load_env(".").expect("load env")
        .load_config("config").expect("load config")
        .with_cache()
        .register_provider(DatabaseServiceProvider)
        .register_provider(RouteServiceProvider)
        .boot().expect("boot");
}
```

`src/main.rs`:

```rust
#[path = "../bootstrap/mod.rs"] mod bootstrap;
mod handlers; mod models; mod requests;
use ravel_facades::{Config, Route};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    bootstrap::app::create_app();
    let router = Route::build();
    let addr = Config::get_or::<String>("server.host", "127.0.0.1".into());
    let port = Config::get_or::<u16>("server.port", 3000);
    println!("Blog running at http://{addr}:{port}");
    ravel_http::server::serve(router, &format!("{addr}:{port}")).await
}
```

### Boot Sequence

`Application::new()` → `.load_env()` → `.load_config()` → `.with_cache()` →
`.register_provider(DatabaseServiceProvider)` → `.register_provider(RouteServiceProvider)` →
`.boot()` → `Route::build()` → `ravel_http::server::serve()`.

---

## Summary

| Concept | v2 API |
|---------|--------|
| Model | `#[derive(Model)]`, `#[model(table = "...", timestamps, soft_deletes)]` |
| Static find | `User::find(db, id)`, `User::find_or_fail(db, id)` |
| Query | `User::query().where_eq(col, val).order_by_desc(col).limit(10).get(db)` |
| Aggregates | `.count()`, `.sum(col)`, `.avg(col)`, `.min(col)`, `.max(col)`, `.group_by(col)` |
| Create | `User::create(serde_json::json!({...}), db)` |
| Save | `user.set_name("Bob").save(db)` (id==0 INSERT, else UPDATE) |
| Delete | `user.delete(db)` (consumptive, soft or hard), `user.force_delete(db)` (hard), `Post::destroy(db, id)` (static) |
| Restore | `trashed.restore(db)` — sets `deleted_at = NULL` |
| Soft Deletes | `all()`, `all_with_trashed()`, `all_only_trashed()`, `query()`, `query_with_trashed()` |
| Eager Loading | `User::query().with("posts").get(db)`, `User::all_with(db, &["posts"])` |
| Relations | `user.posts().where_eq(...).order_by_desc(...).get(db)` |
| BelongsToMany | `user.roles()` — eager-load, `attach()`, `detach()`, `sync()` pivot ops |
| Public JSON | `post.to_public()` / `post.to_public_json()` (hidden fields excluded) |
| Routing | `Route::get/post/put/delete()`, `Route::group()` |
| Validation | `FormRequest` trait, `Validated<T>`, `Rule::Required/Email/Min` |
| Hashing | `Hash::make()`, `Hash::check()` |
| Auth | `Auth::check/guest/login/logout/id()` |
| Sessions | `Session::flash/flashed/get/put()` |
| Responses | `redirect()`, `RavelError`, `axum::Json` |
| Middleware | `Route::middleware()`, `AuthGuard::middleware()` |
| Boot | `Application::new()`, `.register_provider()`, `.boot()`, `Route::build()` |
