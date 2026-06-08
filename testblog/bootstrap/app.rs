// bootstrap/app.rs — Application bootstrap
//
// Registers providers, middleware, and routes using the Ravel Application.

use ravel_core::app::{Application, ServiceProvider};
use ravel_core::container::Container;
use anyhow::Result;

use crate::routes::web;

/// A provider that registers web routes into the container.
pub struct RouteServiceProvider;

impl ServiceProvider for RouteServiceProvider {
    fn register(&self, container: &Container) -> Result<()> {
        // Build routes and store the Router in the container.
        let router = web::routes().build();
        container.instance(router);
        Ok(())
    }

    fn name(&self) -> &str {
        "RouteServiceProvider"
    }
}

/// Create and boot the Ravel application.
pub fn create_app() -> Application {
    Application::new()
        .load_env(".")
        .expect("Failed to load .env")
        .load_config("config")
        .expect("Failed to load config")
        .register_provider(RouteServiceProvider)
        .boot()
        .expect("Failed to boot application")
}
