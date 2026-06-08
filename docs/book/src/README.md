# Introduction

**Ravel** is a Laravel-inspired web framework for Rust, built on [Axum](https://github.com/tokio-rs/axum), [SeaORM](https://www.sea-ql.org/SeaORM/), and [Tokio](https://tokio.rs/).

It brings Laravel's developer experience to the Rust ecosystem: fluent route definitions, static facades, service providers, encrypted sessions, job queues, and a powerful CLI for code scaffolding and database migrations.

## Why Ravel?

Rust offers unparalleled performance and safety. Laravel offers unparalleled developer experience. Ravel bridges the gap:

- **Write less boilerplate.** Define routes with `Route::get("/", handler)` — not manual `Router::new().route(...)` chains.
- **Global facades.** Access `Config`, `Cache`, `Auth`, `Session`, and more from anywhere — no dependency injection ceremony.
- **Service Providers.** Organize your application into modular, testable units with a familiar `register → boot` lifecycle.
- **CLI productivity.** Scaffold controllers, models, migrations, and jobs with a single command.
- **Production-ready.** AES-256-GCM encryption, bcrypt password hashing, CSRF protection, rate limiting, and structured logging out of the box.

## Quick Example

```rust
use ravel_core::app::{Application, ServiceProvider};
use ravel_core::container::Container;
use ravel_facades::{Route, Auth, Session, redirect, abort};
use anyhow::Result;
use axum::response::IntoResponse;

struct RouteServiceProvider;

impl ServiceProvider for RouteServiceProvider {
    fn register(&self, _c: &Container) -> Result<()> {
        Route::get("/", || async { "Hello, Ravel!" });
        Route::get("/dashboard", dashboard);
        Route::post("/login", login);
        Ok(())
    }
    fn name(&self) -> &str { "RouteServiceProvider" }
}

async fn dashboard() -> impl IntoResponse {
    if Auth::guest() {
        return redirect("/login");
    }
    format!("Welcome, user {}!", Auth::id::<String>().unwrap())
}

async fn login() -> Result<impl IntoResponse, ravel_http::error::RavelError> {
    let user = authenticate_user()?;
    Auth::login(&user.id);
    Session::flash("status", "Logged in successfully");
    Ok(redirect("/dashboard"))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Application::new()
        .with_cache()
        .register_provider(RouteServiceProvider)
        .boot()?;

    let router = Route::build();
    ravel_http::server::serve(router, "127.0.0.1:3000").await?;
    Ok(())
}
```

## How to Read This Book

Start with **[Getting Started](./getting-started/installation.md)** to install Ravel and create your first project. Then explore topics as you need them:

- **Core Concepts** — understand the container, facades, and service providers
- **The Basics** — routing, middleware, requests, responses, validation, error handling
- **Security** — authentication, sessions, CSRF, rate limiting
- **Database** — SeaORM integration, migrations, pagination, schema builder
- **Queues** — job dispatch, workers, scheduled tasks
- **Testing** — TestClient, assertions, facade testing patterns
- **CLI** — full Artisan command reference
