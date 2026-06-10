# Ravel

A **Laravel-inspired Rust web framework** built on Axum, SeaORM, and Tokio — bringing Laravel's developer experience to the Rust ecosystem.

## Quick Start

```bash
# Install the CLI
cargo install ravel-cli

# Create a new project
ravel new MyApp
cd my_app

# Start the dev server
ravel serve
```

```
Ravel running at http://127.0.0.1:3000
```

## Hello World

```rust
use ravel_core::app::{Application, ServiceProvider};
use ravel_core::container::Container;
use ravel_facades::Route;
use anyhow::Result;

struct RouteServiceProvider;

impl ServiceProvider for RouteServiceProvider {
    fn register(&self, _container: &Container) -> Result<()> {
        Route::get("/", || async { "Hello, Ravel!" });
        Ok(())
    }

    fn name(&self) -> &str {
        "RouteServiceProvider"
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Application::new()
        .register_provider(RouteServiceProvider)
        .boot()?;

    let router = Route::build();
    ravel_http::server::serve(router, "127.0.0.1:3000").await?;
    Ok(())
}
```

## Features

### Facades — Laravel-Style Static Access

All framework services are accessible via zero-setup static facades:

```rust
use ravel_facades::{Config, Cache, Hash, Crypt, Log, Storage, Route, Auth, Session};

// Configuration
let port: u16 = Config::get_or("server.port", 3000);
let debug: bool = Config::get("app.debug").unwrap_or(false);

// Caching (requires Application::with_cache())
Cache::put("user:1", user_data, Some(Duration::from_secs(3600)));
let cached: Option<User> = Cache::get("user:1");
Cache::forget("user:1");

// Encryption
let encrypted = Crypt::encrypt(b"secret")?;
let decrypted = Crypt::decrypt(&encrypted)?;

// Hashing
let hashed = Hash::make("password")?;
assert!(Hash::check("password", &hashed)?);

// Logging
Log::info!("Server started on port {port}");
Log::error!("Failed to connect to database");

// File Storage
Storage::put("avatars/alice.png", &image_bytes)?;
let file = Storage::get("avatars/alice.png")?;
```

### CLI (ravel-cli)

```
ravel new <name>            Create a new Ravel project
ravel serve                 Start the development server
ravel route:list            List all registered routes
ravel key:generate          Generate an application key

ravel make:controller <n>   Scaffold a Controller
ravel make:middleware <n>   Scaffold a Middleware
ravel make:model <n>        Scaffold a SeaORM entity
ravel make:migration <n>    Scaffold a migration
ravel make:seeder <n>       Scaffold a Seeder
ravel make:provider <n>     Scaffold a ServiceProvider
ravel make:request <n>      Scaffold a FormRequest
ravel make:job <n>          Scaffold a Job

ravel migrate               Run pending migrations
ravel migrate:rollback      Rollback migrations
ravel migrate:refresh       Rollback all + re-apply
ravel migrate:fresh         Drop all tables + re-apply
ravel migrate:status        Show migration status

ravel db:seed               Run database seeders
```

### Service Container (ravel-core)

- **DI Container** — TypeId-based with singleton, transient factory, and pre-built instance support
- **Freeze mechanism** — `container.freeze()` makes reads thread-safe for concurrent HTTP serving
- **Config repository** — Load `.toml` files from `config/`, access via dot-notation keys
- **Env override** — `APP_*` environment variables automatically override TOML config values
- **Service Providers** — Two-phase bootstrap (`register` -> `boot`), inspired by Laravel
- **Event dispatcher** — Lightweight publish/subscribe event system
- **Cache** — In-memory TTL cache (`Cache::put`, `Cache::get`)
- **Encryption** — AES-256-GCM via `Crypt` facade (requires `with_app_key()`)
- **Password hashing** — bcrypt via `Hash` facade
- **Structured logging** — `tracing`-based, configurable via `RAVEL_LOG` env var

```rust
use ravel_core::app::{Application, ServiceProvider};
use ravel_facades::Config;

Application::new()
    .load_env(".")
    .load_config("config")
    .with_cache()
    .with_app_key("base64:...")?
    .register_provider(MyProvider)
    .boot()?;

// After boot(), all facades are globally available
let app_name: String = Config::get("app.name").unwrap();
```

### HTTP Layer (ravel-http)

- **Route Facade** — Global static route registration with Laravel-style API
- **Controller trait** — Organize handlers into controller structs
- **RavelRequest** — Request helpers (`req.query("page")`, `req.header("auth")`)
- **ResponseBuilder** — Fluent response construction (`response().json(data)`, `response().status(201)`)
- **Response Helpers** — `redirect("/login")`, `back()`, `abort(404, "Not found")`
- **FormRequest** — Automatic JSON parsing + validation with 422 responses
- **Validation** — Rules: Required, Min, Max, Email, Regex, In
- **Middleware** — Built-in: error handler, request logger, CORS, Bearer token auth
- **Unified error handling** — `RavelError` enum implements `IntoResponse`, handlers can use `?`
- **ServerBuilder** — Graceful shutdown via Ctrl+C, shared state injection
- **View engine** — Tera template rendering
- **Session** — Encrypted cookie sessions (`Session::put`, `Session::get`, flash messages)
- **Auth** — Session-based authentication (`Auth::check`, `Auth::id`, `Auth::login`)
- **CSRF** — HMAC-based token generation and verification
- **Rate Limiting** — In-memory sliding-window rate limiter
- **Upload** — Multipart file upload with size/MIME validation

```rust
use ravel_facades::Route;
use ravel_facades::{redirect, abort, response};

Route::get("/", || async { "Hello, Ravel!" });
Route::get("/users", list_users);
Route::post("/users", create_user);
Route::group("/admin", || {
    Route::get("/dashboard", admin_dashboard);
    Route::get("/users", admin_users);
});
Route::middleware(log_requests);

let router = Route::build();
```

```rust
use ravel_http::error::RavelError;
use ravel_facades::{Auth, Session, redirect, abort};
use axum::response::IntoResponse;

async fn show_user(id: u32) -> Result<impl IntoResponse, RavelError> {
    if Auth::guest() {
        return Ok(redirect("/login"));
    }

    let user = find_user(id).ok_or_else(|| abort(404, "User not found"))?;

    Session::flash("status", format!("Viewed user {}", id));

    Ok(response().json(user)?)
}
```

### Collection Pipeline

```rust
use ravel_facades::collect;

let active = collect!(users)
    .reject(|u| u.banned)
    .filter(|u| u.age >= 18)
    .sort_by(|u| &u.name)
    .to_vec();

let names: Vec<&str> = collect!(users)
    .map(|u| u.name.as_str())
    .sort()
    .to_vec();
```

### Database & Eloquent ORM (ravel-db-core + ravel-db-seaorm + ravel-eloquent)

- **Multi-connection** — Named connections from `config/database.toml`
- **Migration runner** — CLI-driven with SeaORM migrations
- **Schema Builder** — Programmatic table creation API
- **Eloquent ORM** — Active Record pattern with `#[derive(Model)]`
- **Fluent QueryBuilder** — Type-safe chainable queries on SeaORM `Select<E>`
- **Relationships** — HasMany, BelongsTo, HasOne with lazy-loading `RelationQuery<R>`
- **Pagination** — `Page<T>` with `has_more()`, `last_page()`, `count()`
- **Serialization** — `to_public()` / `to_public_json()` with hidden-field protection

```toml
# config/database.toml
[default]
driver = "postgres"
host = "localhost"
port = 5432
database = "myapp"
username = "user"
password = "secret"
```

```rust
use ravel_eloquent::{Model, ModelExt, ActiveModelExt};

#[derive(Model, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[model(table = "users", timestamps)]
struct User {
    #[model(id)]                                   pub id: i32,
    #[model(string, 254, unique)]                  pub email: String,
    #[model(hidden)]                               pub password: String,
    #[model(has_many)]                             pub posts: HasMany<Post>,
    #[model(belongs_to, from = "team_id", to = "id")] pub team: HasOne<Team>,
}

// Static CRUD
let user = User::find_or_fail(&db, 1).await?;
let all = User::all(&db).await?;
User::destroy(&db, 42).await?;

// Instance methods (consumptive, chainable)
let user = User { id: 0, name: "Alice".into(), email: "a@e.com".into() };
let user = user.save(&db).await?;          // INSERT
let user = user.set_name("Bob").save(&db).await?;  // UPDATE
user.delete(&db).await?;                   // DELETE

// Query builder
let users = User::query()
    .where_eq(UserColumn::Active, true)
    .order_by_desc(UserColumn::CreatedAt)
    .limit(10).get(&db).await?;

// Lazy-loading relations
let posts = user.posts()
    .where_eq(PostColumn::Published, true)
    .latest("created_at")
    .limit(5).get(&db).await?;
let team = user.team().first(&db).await?;

// Safe serialization (excludes #[model(hidden)] fields)
let json = user.to_public_json();  // password NOT included
```

### Job Queue (ravel-support)

- **Async jobs** — `Job` trait with `async fn handle(&self) -> Result<()>`
- **Pluggable drivers** — In-memory (default), Redis, and DB backends
- **Retry with backoff** — Configure `max_attempts()` per job type
- **Dead-letter queue** — Failed jobs tracked with error details and retry support
- **Delayed dispatch** — `Queue::dispatch_later(job, Duration::hours(1))`
- **Task scheduler** — Cron-style recurring tasks with `SchedulerDriver` trait
- **File storage** — `Storage` facade with `LocalDisk` implementation
- **Derive macro** — `#[derive(Job)]` with `#[job(name = "...", queue = "...", max_attempts = N)]`

```rust
use ravel_support::queue::Job;
use ravel_macros::Job;
use ravel_facades::Queue;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Job)]
#[job(name = "send_welcome", queue = "mail", max_attempts = 5)]
struct SendWelcomeEmail { user_id: u32 }

#[async_trait]
impl Job for SendWelcomeEmail {
    async fn handle(&self) -> Result<()> {
        // Send the email...
        Ok(())
    }
}

// Dispatch via facade
Queue::dispatch(SendWelcomeEmail { user_id: 42 })?;
Queue::dispatch_later(job, chrono::Duration::hours(1))?;
```

### Testing (ravel-test)

```rust
use ravel_test::TestClient;
use ravel_facades::{Route, Auth, Session};

#[tokio::test]
async fn test_users_page() {
    let client = TestClient::new(router());
    let resp = client.get("/users").await;
    resp.assert_ok();
    resp.assert_see("Users");
}

#[tokio::test]
async fn test_redirects_when_guest() {
    let client = TestClient::new(router());
    let resp = client.get("/dashboard").await;
    resp.assert_redirect();
}

#[tokio::test]
async fn test_validation_error() {
    let client = TestClient::new(router());
    let resp = client.post_json("/users", r#"{"name":"X"}"#).await;
    resp.assert_unprocessable();
}
```

## Project Structure

```
ravel/
├── crates/
│   ├── ravel-cli/            CLI tool (clap)
│   ├── ravel-core/            DI container, config, env, events, cache, crypt, hash, log
│   ├── ravel-http/            Route builder, server, middleware, session, auth, csrf
│   ├── ravel-db-core/         Database abstraction traits, schema builder
│   ├── ravel-db-seaorm/       SeaORM implementation: connection, migration, pagination
│   ├── ravel-eloquent/        Eloquent ORM: Active Record, QueryBuilder, relations
│   ├── ravel-eloquent-macros/ #[derive(Model)] proc-macro
│   ├── ravel-generator/       Code scaffolding (Tera templates)
│   ├── ravel-support/         Queue, scheduler, storage
│   ├── ravel-macros/          Proc-macros (#[derive(Job)])
│   ├── ravel-facades/         Laravel-style static facades (Route, Config, Cache, etc.)
│   └── ravel-test/            TestClient and response assertions
└── testblog/                  Reference application + integration tests
```

## Quick Reference — Facade API

### Configuration & Environment
| Method | Description |
|--------|-------------|
| `Config::get::<T>(key)` | Get typed config value |
| `Config::get_or::<T>(key, default)` | Get with fallback |
| `Config::has(key)` | Check key existence |
| `env("KEY")` | Get environment variable |
| `env_or("KEY", "default")` | Get env var with fallback |

### Caching & Encryption
| Method | Description |
|--------|-------------|
| `Cache::put(key, value, ttl)` | Store with optional TTL |
| `Cache::get::<T>(key)` | Retrieve typed value |
| `Cache::has(key)` / `Cache::forget(key)` | Check / remove |
| `Crypt::encrypt(data)` / `Crypt::decrypt(data)` | AES-256-GCM encrypt/decrypt |
| `Hash::make(pw)` / `Hash::check(pw, hash)` | bcrypt password ops |

### Routing
| Method | Description |
|--------|-------------|
| `Route::get(path, handler)` | Register GET route |
| `Route::post / put / delete / patch` | Other HTTP verbs |
| `Route::group(prefix, \|\| { ... })` | Route grouping |
| `Route::middleware(fn)` | Attach middleware |
| `Route::build()` | Build final Axum Router |

### Request & Response
| Method | Description |
|--------|-------------|
| `request().query::<T>(key)` | Query parameter |
| `request().header(name)` | Request header |
| `request().wants_json()` | Content negotiation |
| `response().json(data)` | JSON response |
| `redirect(url)` | 302 redirect |
| `back()` | Redirect to Referer |
| `abort(404, "msg")` | HTTP error response |

### Auth & Session
| Method | Description |
|--------|-------------|
| `Auth::check()` / `Auth::guest()` | Authentication status |
| `Auth::id::<T>()` | Get current user ID |
| `Auth::login(id)` / `Auth::logout()` | Login/logout |
| `Session::get::<T>(key)` / `Session::put(key, val)` | Session read/write |
| `Session::flash(key, val)` / `Session::flashed::<T>(key)` | Flash messages |

### Utilities
| Method | Description |
|--------|-------------|
| `collect!(vec![...]).map(...).filter(...).to_vec()` | Collection pipeline |
| `config_path("app.toml")` | Path helper |
| `database_path("migrations")` | Path helper |
| `storage_path("logs/app.log")` | Path helper |
| `now()` | Current UTC timestamp |

## Requirements

- Rust 1.82+ (edition 2024)
- PostgreSQL / MySQL / SQLite (for database features)

## License

MIT
