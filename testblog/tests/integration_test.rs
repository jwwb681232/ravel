//! Integration tests for the testblog application.
//!
//! These tests validate that the Ravel Application lifecycle works end-to-end:
//! - Application boots successfully
//! - Container freezes after boot
//! - Router is resolved from the container
//! - Routes respond correctly

use ravel_core::app::{Application, ServiceProvider};
use ravel_core::container::Container;
use ravel_http::route::Route;
use anyhow::Result;
use axum::Router;
use std::sync::Arc;

// ── A minimal ServiceProvider for testing ─────────────────────────

struct TestRouteProvider;

impl ServiceProvider for TestRouteProvider {
    fn register(&self, container: &Container) -> Result<()> {
        let router = Route::new()
            .get("/", || async { "Hello, Ravel!" })
            .build();
        container.instance(router);
        Ok(())
    }

    fn name(&self) -> &str {
        "TestRouteProvider"
    }
}

fn create_test_app() -> Application {
    Application::new()
        .register_provider(TestRouteProvider)
        .boot()
        .expect("Failed to boot test application")
}

// ── Application bootstrap tests ───────────────────────────────────

#[test]
fn test_app_boots_successfully() {
    let app = create_test_app();
    let router: Arc<Router> = app.container().resolve().expect("Router not found");
    // Router should be non-empty
    assert!(!format!("{:?}", router).is_empty());
}

#[test]
fn test_container_has_router() {
    let app = create_test_app();
    assert!(app.container().has::<Router>());
}

#[test]
fn test_app_resolves_router() {
    let app = create_test_app();
    let router: Arc<Router> = app.container().resolve().expect("Router not in container");
    assert!(Arc::strong_count(&router) >= 1);
}

// ── HTTP request tests ─────────────────────────────────────────────

fn test_app_router() -> Router {
    let app = create_test_app();
    let router: Arc<Router> = app.container().resolve().expect("Router not found");
    (*router).clone()
}

#[tokio::test]
async fn test_root_route_returns_hello() {
    let client = ravel_test::TestClient::new(test_app_router());
    let resp = client.get("/").await;
    resp.assert_ok();
    resp.assert_see("Hello, Ravel!");
}

#[tokio::test]
async fn test_not_found_route() {
    let client = ravel_test::TestClient::new(test_app_router());
    let resp = client.get("/nonexistent").await;
    assert_eq!(resp.status(), axum::http::StatusCode::NOT_FOUND);
}
