use ravel_core::app::Application;
use ravel_facades::Route;
use ravel_http::server;

mod routes;
mod app;

use app::Providers::app_service_provider::AppServiceProvider;
use app::Providers::route_service_provider::RouteServiceProvider;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _app = Application::new()
        .load_env(".")?
        .load_config("config")?
        .with_cache()
        .with_app_key("base64:YOUR_APP_KEY_HERE")?
        .register_provider(AppServiceProvider)
        .register_provider(RouteServiceProvider)
        .boot()?;

    let router = Route::build();

    let host = "127.0.0.1:3000";
    println!("MyApp running at http://{host}");
    server::serve(router, host).await?;

    Ok(())
}
