# Installation

## Prerequisites

- **Rust** 1.82+ (edition 2024). Install via [rustup](https://rustup.rs):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

- **PostgreSQL**, **MySQL**, or **SQLite** — only needed if you use database features.

## Install the CLI

```bash
cargo install ravel-cli
```

Verify the installation:

```bash
ravel --version
```

## Create a New Project

```bash
ravel new MyApp
cd my_app
```

This scaffolds a complete project structure including routes, config, and a working Hello World endpoint.

## Start the Dev Server

```bash
ravel serve
```

```
Ravel running at http://127.0.0.1:3000
```

Open `http://127.0.0.1:3000` — you should see "Hello, Ravel!".

## Manual Setup

If you prefer to add Ravel to an existing Rust project:

```toml
[dependencies]
ravel-core = "0.1"
ravel-http = "0.1"
ravel-facades = "0.1"
tokio = { version = "1", features = ["full"] }
anyhow = "1"
```

```rust
// src/main.rs
use ravel_core::app::{Application, ServiceProvider};
use ravel_core::container::Container;
use ravel_facades::Route;
use anyhow::Result;

struct RouteServiceProvider;

impl ServiceProvider for RouteServiceProvider {
    fn register(&self, _c: &Container) -> Result<()> {
        Route::get("/", || async { "Hello, Ravel!" });
        Ok(())
    }
    fn name(&self) -> &str { "RouteServiceProvider" }
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
