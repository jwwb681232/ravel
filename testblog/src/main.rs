#[path = "../bootstrap/mod.rs"]
mod bootstrap;

use ravel_core::app::with_app;
use ravel_facades::{Config, Route};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = bootstrap::app::create_app();

    let router = Route::build();

    let addr = Config::get_or::<String>("server.host", "127.0.0.1".into());
    let port = Config::get_or::<u16>("server.port", 3000);
    let bind_addr = format!("{addr}:{port}");

    println!("Ravel running at http://{bind_addr}");

    with_app(app, ravel_http::server::serve(router, &bind_addr)).await?;
    Ok(())
}
