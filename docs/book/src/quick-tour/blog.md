# Quick Tour: Building a Blog

A complete blog with user registration, login, CRUD posts, and logout. All code uses the facade API and `#[derive(Model)]` ORM.

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
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
tokio = { version = "1", features = ["full"] }
anyhow = "1"
sea-orm = { version = "2.0.0-rc.40", features = ["sqlx-sqlite", "runtime-tokio-rustls", "macros"] }
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

Migrations (`ravel make:migration`):

```rust
// users: schema.create("users", |t| { t.id(); t.string("name",255);
//   t.string("email",255).unique(); t.string("password",255); t.timestamps(); });
// posts: schema.create("posts", |t| { t.id(); t.string("title",255);
//   t.text("body"); t.integer("user_id"); t.timestamps(); });
```

Run `ravel migrate`.

---

## 3. User Registration

### Model

```rust
use ravel_eloquent::Model;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Model, Serialize, Deserialize)]
#[model(table = "users")]
pub struct User {
    #[model(id)] pub id: i32,
    #[model(string, 255)] pub name: String,
    #[model(string, 255, unique)] pub email: String,
    #[model(hidden)] pub password: String,
    #[model(timestamps)] pub created_at: chrono::NaiveDateTime,
    #[model(timestamps)] pub updated_at: chrono::NaiveDateTime,
}
```

`#[derive(Model)]` generates `UserColumn`, `UserPublic`, `User::query()`, and `User::r#where()`.

### Form Request & Handler

```rust
use ravel_http::form_request::{FormRequest, Validated};
use ravel_http::validation::{FieldRule, Rule};
use axum::response::IntoResponse;
use ravel_facades::{Hash, redirect, Session};
use ravel_http::error::RavelError;

#[derive(Debug, Deserialize)]
pub struct RegisterRequest { pub name: String, pub email: String, pub password: String }

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
    User::create(serde_json::json!({"name":req.name,"email":req.email,"password":hashed}), &db())
        .await.map_err(|e| RavelError::internal(e.to_string()))?;
    Session::flash("status", "Registration successful! Please log in.");
    Ok(redirect("/login"))
}
```

## 4. Login

```rust
#[derive(Debug, Deserialize)]
pub struct LoginRequest { pub email: String, pub password: String }

impl FormRequest for LoginRequest {
    fn rules() -> Vec<FieldRule> {
        vec![FieldRule::new("email", vec![Rule::Required, Rule::Email]),
             FieldRule::new("password", vec![Rule::Required])]
    }
}

use ravel_facades::Auth;

pub async fn login(
    Validated(req): Validated<LoginRequest>,
) -> Result<impl IntoResponse, RavelError> {
    let user = find_user_by_email(&req.email).await
        .ok_or(RavelError::unauthorized("Invalid credentials"))?;
    if !Hash::check(&req.password, &user.password)? {
        return Err(RavelError::unauthorized("Invalid credentials"));
    }
    Auth::login(&user.id);
    Session::flash("status", "Welcome back!");
    Ok(redirect("/posts"))
}
```

Flow: `Validated<T>` → find user → `Hash::check()` → `Auth::login()` → flash → redirect.

---

## 5. Posts CRUD

### Model

```rust
#[derive(Debug, Clone, Model, Serialize, Deserialize)]
#[model(table = "posts")]
pub struct Post {
    #[model(id)] pub id: i32,
    #[model(string, 255)] pub title: String,
    #[model(text)] pub body: String,
    #[model(integer)] pub user_id: i32,
    #[model(timestamps)] pub created_at: chrono::NaiveDateTime,
    #[model(timestamps)] pub updated_at: chrono::NaiveDateTime,
}
impl Post { pub fn owned_by(&self, uid: i32) -> bool { self.user_id == uid } }
```

### List & Show

```rust
pub async fn index() -> Result<impl IntoResponse, RavelError> {
    let sql = Post::query().order_by("created_at", "DESC").to_select_sql();
    Ok(axum::Json(serde_json::json!({ "sql": sql }))) // render via View
}

pub async fn show(Path(id): Path<i32>) -> Result<impl IntoResponse, RavelError> {
    let post = find_one::<Post>(&Post::r#where("id", id).to_select_sql()).await
        .ok_or(RavelError::not_found("Post not found"))?;
    Ok(axum::Json(serde_json::json!(post)))
}
```

### Create & Store

```rust
pub async fn create() -> impl IntoResponse {
    if Auth::guest() { return redirect("/login"); }
    "Create Post form"
}

#[derive(Debug, Deserialize)]
pub struct StorePostRequest { pub title: String, pub body: String }

impl FormRequest for StorePostRequest {
    fn rules() -> Vec<FieldRule> {
        vec![FieldRule::new("title", vec![Rule::Required, Rule::Min(3)]),
             FieldRule::new("body", vec![Rule::Required, Rule::Min(10)])]
    }
}

pub async fn store(
    Validated(req): Validated<StorePostRequest>,
) -> Result<impl IntoResponse, RavelError> {
    let uid: i32 = Auth::id().ok_or(RavelError::unauthorized("Not logged in"))?;
    Post::create(serde_json::json!({"title":req.title,"body":req.body,"user_id":uid}), &db())
        .await.map_err(|e| RavelError::internal(e.to_string()))?;
    Session::flash("status", "Post created!");
    Ok(redirect("/posts"))
}
```

### Edit & Update

```rust
pub async fn edit(Path(id): Path<i32>) -> Result<impl IntoResponse, RavelError> {
    if Auth::guest() { return Ok(redirect("/login")); }
    let post = find_one::<Post>(&Post::r#where("id", id).to_select_sql()).await
        .ok_or(RavelError::not_found("Post not found"))?;
    if !post.owned_by(Auth::id::<i32>().unwrap()) {
        return Err(RavelError::forbidden("Not your post"));
    }
    Ok(axum::Json(serde_json::json!({ "post": post })))
}

pub async fn update(Path(id): Path<i32>, Validated(req): Validated<StorePostRequest>,
) -> Result<impl IntoResponse, RavelError> {
    let mut post = find_one::<Post>(&Post::r#where("id", id).to_select_sql()).await
        .ok_or(RavelError::not_found("Post not found"))?;
    if !post.owned_by(Auth::id::<i32>().unwrap()) {
        return Err(RavelError::forbidden("Not your post"));
    }
    post.update(serde_json::json!({"title":req.title,"body":req.body}), &db()).await
        .map_err(|e| RavelError::internal(e.to_string()))?;
    Session::flash("status", "Post updated!");
    Ok(redirect("/posts"))
}
```

### Delete

```rust
pub async fn destroy(Path(id): Path<i32>) -> Result<impl IntoResponse, RavelError> {
    let post = find_one::<Post>(&Post::r#where("id", id).to_select_sql()).await
        .ok_or(RavelError::not_found("Post not found"))?;
    if !post.owned_by(Auth::id::<i32>().unwrap()) {
        return Err(RavelError::forbidden("Not your post"));
    }
    post.delete(&db()).await.map_err(|e| RavelError::internal(e.to_string()))?;
    Session::flash("status", "Post deleted!");
    Ok(redirect("/posts"))
}
```

### Relation

```rust
use ravel_eloquent::relations::RelationBuilder;
// All posts by user: RelationBuilder::<Post>::has_many("posts", "user_id", uid).to_sql()

use ravel_eloquent::HasRelations;
let sql = user.has_many::<Post>("posts", "user_id", user.id.into()).to_sql();
```

---

## 6. Auth Guards

### Per-Handler

```rust
pub async fn dashboard() -> impl IntoResponse {
    if Auth::guest() { Session::flash("status","Please log in."); return redirect("/login"); }
    format!("Welcome, user {}!", Auth::id::<i32>().unwrap_or(0))
}
```

### AuthGuard Middleware

```rust
use ravel_http::auth::AuthGuard;
Route::group("/admin", || { Route::get("/dashboard", dashboard); });
Route::middleware(AuthGuard::middleware()); // protects subsequent routes (401 on missing cookie)
```

### Flash Messages

```rust
Session::flash("status", "Done!");                      // write (next request)
let msg: Option<String> = Session::flashed("status");  // read-once
```

---

## 7. Logout

```rust
pub async fn logout() -> impl IntoResponse {
    Auth::logout();
    Session::flash("status", "Logged out.");
    redirect("/")
}
// Register as POST: Route::post("/logout", handlers::auth::logout);
```

---

## 8. Full Router Setup

`bootstrap/app.rs`:

```rust
use ravel_core::app::{Application, ServiceProvider};
use ravel_core::container::Container;
use ravel_facades::Route;
use ravel_http::auth::AuthGuard;

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

`Application::new()` → `.load_env()` → `.load_config()` → `.with_cache()` → `.register_provider()` → `.boot()` calls `register()` then `boot()` on all providers, freezes the container, stores global `APP` → `Route::build()` extracts the `Router` → `ravel_http::server::serve()` starts listening.

---

## Summary

| Concept | API |
|---------|------|
| Routing | `Route::get/post/put/delete()`, `Route::group()` |
| Validation | `FormRequest` trait, `Validated<T>`, `Rule::Required/Email/Min` |
| Hashing | `Hash::make()`, `Hash::check()` |
| Authentication | `Auth::check/guest/login/logout/id()` |
| Sessions | `Session::flash/flashed/get/put()` |
| Models | `#[derive(Model)]`, `User::query()`, `User::r#where()`, `Post::create/update/delete()` |
| Relations | `RelationBuilder::has_many()`, `HasRelations` |
| Config | `Config::get()`, `Config::get_or()` |
| Responses | `redirect()`, `back()`, `RavelError` |
| Middleware | `Route::middleware()`, `AuthGuard::middleware()` |
| Boot | `Application::new()`, `.load_config()`, `.register_provider()`, `.boot()`, `Route::build()`, `ravel_http::server::serve()` |
