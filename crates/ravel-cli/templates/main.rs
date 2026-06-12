#![allow(dead_code)]

use ravel_core::app::{Application, set_app_global};
use ravel_facades::{Config, Route};
use ravel_http::server;

#[path = "../app/mod.rs"]
mod app;
#[path = "../routes/mod.rs"]
mod routes;

use app::providers::app_service_provider::AppServiceProvider;
use app::providers::route_service_provider::RouteServiceProvider;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Resolve config directory from the project root (where this crate
    // was compiled from). Using `env!("CARGO_MANIFEST_DIR")` keeps the
    // path correct even if the binary is launched from elsewhere
    // (e.g. `cargo run` invoked from the workspace root).
    let config_dir = format!("{}/config", env!("CARGO_MANIFEST_DIR"));

    let app = Application::new()
        .load_config(&config_dir)?
        .with_cache()
        .with_app_key_from_env()?
        .register_provider(AppServiceProvider)
        .register_provider(RouteServiceProvider)
        .boot()?;

    set_app_global(app);

    let router = Route::build();

    // Read bind address from config. Keys live under the `app.*`
    // namespace because the file is `config/app.toml`; the inner
    // `[server]` section becomes `app.server.*`.
    let host: String = Config::get_or("app.server.host", "127.0.0.1".to_string());
    let port: u16    = Config::get_or("app.server.port", 3000);
    let bind = format!("{host}:{port}");

    println!("{{name}} running at http://{bind}");
    server::serve(router, &bind).await?;

    Ok(())
}
