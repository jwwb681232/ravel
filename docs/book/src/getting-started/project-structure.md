# Project Structure

When you run `ravel new MyApp`, the CLI scaffolds this directory tree:

```
my_app/
├── Cargo.toml
├── .env                          # Environment variables
├── config/
│   └── app.toml                  # Application configuration
├── app/
│   ├── Http/
│   │   ├── Controllers/          # HTTP controllers
│   │   ├── Middleware/            # Custom middleware
│   │   └── Requests/             # Form request validation
│   ├── Models/                   # Database models
│   ├── Providers/                # Service providers
│   ├── Services/                 # Business logic services
│   └── Jobs/                     # Queue jobs
├── bootstrap/
│   └── app.rs                    # Application bootstrap
├── routes/
│   └── web.rs                    # Web route definitions
├── database/
│   ├── migrations/               # Database migrations
│   └── seeders/                  # Database seeders
├── storage/
│   └── logs/                     # Log files, uploads
├── tests/
│   └── integration_test.rs
└── src/
    ├── main.rs                   # Entry point
    └── bin/
        ├── migrate.rs            # Migration binary
        └── seed.rs               # Seeder binary
```

## Key Files

### bootstrap/app.rs

The application bootstrap file. This is where you register service providers, load config, and boot the framework:

```rust
use ravel_core::app::{Application, ServiceProvider};
use ravel_core::container::Container;
use ravel_facades::Route;
use anyhow::Result;

pub struct RouteServiceProvider;

impl ServiceProvider for RouteServiceProvider {
    fn register(&self, _c: &Container) -> Result<()> {
        // Register routes here
        Route::get("/", || async { "Hello!" });
        Ok(())
    }
    fn name(&self) -> &str { "RouteServiceProvider" }
}

pub fn create_app() {
    Application::new()
        .load_env(".")
        .expect("Failed to load .env")
        .load_config("config")
        .expect("Failed to load config")
        .with_cache()
        .register_provider(RouteServiceProvider)
        .boot()
        .expect("Failed to boot");
}
```

### src/main.rs

The entry point:

```rust
use ravel_core::app::APP;
use ravel_facades::{Config, Route};

mod bootstrap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    bootstrap::app::create_app();

    let router = Route::build();
    let host = Config::get_or::<String>("server.host", "127.0.0.1".into());
    let port = Config::get_or::<u16>("server.port", 3000);

    println!("Ravel running at http://{host}:{port}");
    ravel_http::server::serve(router, &format!("{host}:{port}")).await?;
    Ok(())
}
```

### config/app.toml

Application configuration in TOML format. Values are accessible via `Config::get()`:

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
