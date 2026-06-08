# Quick Start

This guide walks through building a simple blog-like application with Ravel.

## Your First Route

Routes are registered via the `Route` facade inside a service provider:

```rust
use ravel_core::app::{Application, ServiceProvider};
use ravel_core::container::Container;
use ravel_facades::Route;
use anyhow::Result;

struct RouteServiceProvider;

impl ServiceProvider for RouteServiceProvider {
    fn register(&self, _c: &Container) -> Result<()> {
        Route::get("/", || async { "Welcome to my blog!" });
        Route::get("/about", || async { "About page" });
        Ok(())
    }
    fn name(&self) -> &str { "RouteServiceProvider" }
}
```

## Bootstrap the Application

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Application::new()
        .load_env(".")
        .load_config("config")
        .with_cache()
        .register_provider(RouteServiceProvider)
        .boot()?;

    let router = Route::build();
    ravel_http::server::serve(router, "127.0.0.1:3000").await?;
    Ok(())
}
```

## Configuration

Create `config/app.toml`:

```toml
[app]
name = "MyBlog"
env = "local"
debug = true

[server]
host = "127.0.0.1"
port = 3000
```

Access config anywhere via the `Config` facade:

```rust
use ravel_facades::Config;

async fn status() -> String {
    let name: String = Config::get("app.name").unwrap();
    let debug: bool = Config::get_or("app.debug", false);
    format!("{name} running in debug={debug}")
}
```

## Route Groups

```rust
Route::group("/posts", || {
    Route::get("/", list_posts);
    Route::get("/{id}", show_post);
    Route::post("/", create_post);
});

Route::group("/admin", || {
    Route::get("/dashboard", admin_dashboard);
    Route::get("/posts", admin_posts);
});
```

## Middleware

```rust
use ravel_facades::Route;
use ravel_http::middleware;

Route::get("/", home);
Route::get("/dashboard", dashboard);

// Apply middleware to a group
Route::group("/admin", || {
    Route::get("/dashboard", admin_dashboard);
});
Route::middleware(middleware::log_requests);

let router = Route::build();
```

## Next Steps

- Read [Service Container](../core-concepts/service-container.md) to understand dependency injection
- Read [Routing](../the-basics/routing.md) for the full route API
- Read [Facades](../core-concepts/facades.md) to learn about static access
