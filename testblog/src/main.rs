#[path = "../bootstrap/mod.rs"]
mod bootstrap;
#[path = "../routes/mod.rs"]
mod routes;

use axum::Router;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Bootstrap the Ravel application — loads config, env, registers providers,
    // and freezes the container for thread-safe access.
    let app = bootstrap::app::create_app();

    // Resolve the Axum Router that was registered by RouteServiceProvider.
    let router: Arc<Router> = app
        .container()
        .resolve()
        .expect("Router not found in container");

    let addr = app
        .config()
        .get::<String>("server.host")
        .unwrap_or_else(|| "127.0.0.1".into());
    let port = app.config().get::<u16>("server.port").unwrap_or(3000);
    let bind_addr = format!("{addr}:{port}");

    println!("Ravel running at http://{bind_addr}");

    // Start the Axum server.
    ravel_http::server::serve((*router).clone(), &bind_addr).await?;

    Ok(())
}
