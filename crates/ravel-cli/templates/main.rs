use ravel_core::app::Application;
use ravel_facades::Route;
use ravel_http::server;

#[path = "../app/mod.rs"]
mod app;
#[path = "../routes/mod.rs"]
mod routes;

use app::providers::app_service_provider::AppServiceProvider;
use app::providers::route_service_provider::RouteServiceProvider;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _app = Application::new()
        .load_env(env!("CARGO_MANIFEST_DIR"))?
        .load_config("config")?
        .with_cache()
        .with_app_key_from_env()?
        .register_provider(AppServiceProvider)
        .register_provider(RouteServiceProvider)
        .boot()?;

    let router = Route::build();

    let host = "127.0.0.1:3000";
    println!("{{name}} running at http://{host}");
    server::serve(router, host).await?;

    Ok(())
}
