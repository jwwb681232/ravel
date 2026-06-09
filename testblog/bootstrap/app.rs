// bootstrap/app.rs — Application bootstrap
use anyhow::Result;
use ravel_core::app::{Application, ServiceProvider};
use ravel_core::container::Container;
use ravel_facades::Route;
use std::sync::Arc;

pub struct RouteServiceProvider;

impl ServiceProvider for RouteServiceProvider {
    fn register(&self, _container: &Container) -> Result<()> {
        Route::get("/", || async { "Hello, Ravel!" });
        Ok(())
    }

    fn name(&self) -> &str {
        "RouteServiceProvider"
    }
}

/// Create and boot the Ravel application.
pub fn create_app() -> Arc<Application> {
    Application::new()
        .load_env(".")
        .expect("Failed to load .env")
        .load_config("config")
        .expect("Failed to load config")
        .register_provider(RouteServiceProvider)
        .boot()
        .expect("Failed to boot application")
}
