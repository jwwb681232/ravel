# Project Structure

When you run `ravel new MyApp`, the CLI scaffolds this directory tree — modelled after Laravel's layout:

```
my_app/
├── Cargo.toml
├── .env                              # Environment variables
├── .env.example                      # Template for .env
├── config/
│   └── app.toml                      # Application configuration
├── app/
│   ├── Http/
│   │   ├── Controllers/              # HTTP request handlers
│   │   ├── Middleware/                # Request filtering middleware
│   │   └── Requests/                 # FormRequest validation
│   ├── Models/                       # Eloquent ORM models
│   ├── Providers/                    # Service providers (boot lifecycle)
│   ├── Services/                     # Business logic / reusable services
│   └── Jobs/                         # Queue jobs
├── bootstrap/
│   └── app.rs                        # Application factory
├── routes/
│   └── web.rs                        # Route definitions
├── database/
│   ├── migrations/                   # Schema migration files
│   └── seeders/                      # Database seeders
├── storage/                          # Logs, uploads, cache files
├── tests/                            # Integration tests
└── src/
    ├── main.rs                       # Entry point
    └── bin/
        ├── migrate.rs                # CLI migration runner
        └── seed.rs                   # CLI seeder runner
```

## Directory Quick Reference

| Directory | Purpose | Convention |
|-----------|---------|------------|
| **`app/Http/Controllers/`** | HTTP request handlers | One file per resource: `UserController.rs`, `PostController.rs` |
| **`app/Http/Middleware/`** | Request/response filters | `Auth.rs`, `Cors.rs`, `LogRequest.rs` |
| **`app/Http/Requests/`** | FormRequest validation structs | `CreateUserRequest.rs`, `LoginRequest.rs` |
| **`app/Models/`** | Eloquent `#[derive(Model)]` structs | `User.rs`, `Post.rs`, `Comment.rs` |
| **`app/Providers/`** | Service providers (register + boot) | `RouteServiceProvider.rs`, `AppServiceProvider.rs` |
| **`app/Jobs/`** | Queue job structs | `SendWelcomeEmail.rs`, `ProcessImage.rs` |
| **`app/Services/`** | Business logic / reusable services | `PaymentService.rs`, `NotificationService.rs` |
| **`routes/`** | Route registration files | `web.rs`, `api.rs` |
| **`database/migrations/`** | Schema migration files | Timestamped: `2024_01_01_000000_create_users.rs` |
| **`database/seeders/`** | Data population scripts | `UserSeeder.rs`, `RoleSeeder.rs` |

## Key Files

### bootstrap/app.rs

The application factory. Register providers, load config, enable optional features:

```rust
use ravel_core::app::{Application, ServiceProvider};
use ravel_core::container::Container;
use ravel_facades::Route;
use anyhow::Result;

struct RouteServiceProvider;

impl ServiceProvider for RouteServiceProvider {
    fn register(&self, _c: &Container) -> Result<()> {
        Route::get("/", || async { "Hello, Ravel! 🚀" });
        Ok(())
    }
    fn name(&self) -> &str { "RouteServiceProvider" }
}

pub fn create_app() -> std::sync::Arc<Application> {
    Application::new()
        .load_env(".")
        .expect("Failed to load .env")
        .load_config("config")
        .expect("Failed to load config")
        .with_cache()
        .with_queue()
        .register_provider(RouteServiceProvider)
        .boot()
        .expect("Failed to boot")
}
```

### src/main.rs

```rust
use ravel_core::app::with_app;
use ravel_facades::{Config, Route};

mod bootstrap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = bootstrap::app::create_app();
    let router = Route::build();

    let host = Config::get_or::<String>("server.host", "127.0.0.1".into());
    let port = Config::get_or::<u16>("server.port", 3000);
    let bind = format!("{host}:{port}");

    println!("Ravel running at http://{bind}");
    with_app(app, ravel_http::server::serve(router, &bind)).await?;
    Ok(())
}
```

### config/app.toml

```toml
[app]
name = "MyApp"
env = "local"
debug = true
url = "http://localhost:3000"

[server]
host = "127.0.0.1"
port = 3000
```
