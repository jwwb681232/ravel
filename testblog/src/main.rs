#[path = "../bootstrap/mod.rs"]
mod bootstrap;

use ravel_core::app::APP;
use ravel_facades::{Config, Route};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    bootstrap::app::create_app();

    let router = Route::build();

    let _app = APP.get().expect("Application not booted");
    let addr = Config::get_or::<String>("server.host", "127.0.0.1".into());
    let port = Config::get_or::<u16>("server.port", 3000);
    let bind_addr = format!("{addr}:{port}");

    println!("Ravel running at http://{bind_addr}");

    ravel_http::server::serve(router, &bind_addr).await?;
    Ok(())
}
