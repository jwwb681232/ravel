# Introduction

**Ravel** is a Laravel-inspired web framework for Rust, built on [Axum](https://github.com/tokio-rs/axum), [SeaORM](https://www.sea-ql.org/SeaORM/), and [Tokio](https://tokio.rs/). It brings Laravel's developer experience to the Rust ecosystem.

## Why Ravel?

Rust offers performance and safety. Laravel offers developer experience. Ravel bridges the gap:

- **Static facades** — `Config::get()`, `Route::get("/", handler)`, `Cache::put(...)`. No dependency injection ceremony.
- **Service Providers** — Modular, testable application bootstrap with `register → boot` lifecycle.
- **Eloquent ORM** — `#[derive(Model)]` with fluent query builder, relationships, soft deletes, and eager loading.
- **FormRequest validation** — Automatic JSON parsing + validation with 422 responses, database rules (`Unique`, `Exists`), and custom messages.
- **Queue system** — In-memory (dev) and Redis (production) drivers with retry, delay, and failed-job tracking.
- **Session management** — Encrypted cookie (default) or Redis-backed sessions for horizontal scaling.
- **CLI scaffolding** — `ravel make:model`, `ravel make:controller`, `ravel migrate` — Laravel-style code generation.
- **Production-ready** — AES-256-GCM encryption, bcrypt hashing, CSRF, rate limiting, structured logging.

## Quick Example

```rust
use ravel_core::app::{Application, ServiceProvider};
use ravel_core::container::Container;
use ravel_facades::{Route, Config};
use anyhow::Result;
use axum::response::IntoResponse;

struct RouteServiceProvider;

impl ServiceProvider for RouteServiceProvider {
    fn register(&self, _c: &Container) -> Result<()> {
        Route::get("/", || async { "Hello, Ravel! 🚀" });
        Route::get("/health", health);
        Ok(())
    }
    fn name(&self) -> &str { "RouteServiceProvider" }
}

async fn health() -> impl IntoResponse {
    let port: u16 = Config::get_or("server.port", 3000);
    format!("Running on port {}", port)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = Application::new()
        .load_env(".")?
        .load_config("config")?
        .register_provider(RouteServiceProvider)
        .boot()?;

    let router = Route::build();
    ravel_http::server::serve(router, "127.0.0.1:3000").await?;
    Ok(())
}
```

## How to Read This Book

Start with **[Installation](./getting-started/installation.md)** to install Ravel and create your first project.

- **Getting Started** — installation, first project, directory structure
- **Core Concepts** — service container, facades, service providers, error handling
- **The Basics** — routing, middleware, requests, responses, validation
- **Security** — sessions, authentication, CSRF, rate limiting
- **Database** — Eloquent ORM, query builder, relationships, migrations, pagination
- **Queues** — job dispatch, Redis workers, scheduling
- **Testing** — `TestClient`, `#[ravel::test]` macro, facade testing
- **CLI** — `ravel make`, `ravel migrate`, `ravel serve`
