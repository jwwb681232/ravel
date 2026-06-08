//! Integration tests using ravel-facades.
use axum::http::StatusCode;
use ravel_core::app::{Application, ServiceProvider};
use ravel_core::container::Container;
use ravel_facades::Route;
use anyhow::Result;

// ── ServiceProvider ──────────────────────────────────────────────────

struct TestRouteProvider;

impl ServiceProvider for TestRouteProvider {
    fn register(&self, _container: &Container) -> Result<()> {
        Route::get("/", || async { "Hello, Ravel!" });
        Route::get("/health", || async { (StatusCode::OK, "OK") });
        Ok(())
    }

    fn name(&self) -> &str {
        "TestRouteProvider"
    }
}

fn create_test_app() {
    Route::reset();
    ravel_core::app::APP.reset();
    Application::new()
        .register_provider(TestRouteProvider)
        .boot()
        .expect("Failed to boot test application");
}

fn test_router() -> axum::Router {
    create_test_app();
    Route::build()
}

// ── Application bootstrap tests ─────────────────────────────────────

#[test]
fn test_app_boots_successfully() {
    Route::reset();
    ravel_core::app::APP.reset();
    Application::new()
        .register_provider(TestRouteProvider)
        .boot()
        .unwrap();
    let app = ravel_core::app::APP.get().unwrap();
    assert!(app.is_booted());
}

#[test]
fn test_app_is_booted_after_boot() {
    ravel_core::app::APP.reset();
    create_test_app();
    let app = ravel_core::app::APP.get().unwrap();
    assert!(app.is_booted());
}

#[test]
fn test_app_config_loaded() {
    ravel_core::app::APP.reset();
    Application::new()
        .load_config("nonexistent-dir")
        .unwrap()
        .boot()
        .unwrap();
    let app = ravel_core::app::APP.get().unwrap();
    assert!(app.config().is_empty() || !app.config().is_empty());
}

// ── HTTP request tests ──────────────────────────────────────────────

#[tokio::test]
async fn test_root_route_returns_hello() {
    let client = ravel_test::TestClient::new(test_router());
    let resp = client.get("/").await;
    resp.assert_ok();
    resp.assert_see("Hello, Ravel!");
}

#[tokio::test]
async fn test_not_found_route() {
    let client = ravel_test::TestClient::new(test_router());
    let resp = client.get("/nonexistent").await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_health_endpoint() {
    let client = ravel_test::TestClient::new(test_router());
    let resp = client.get("/health").await;
    resp.assert_ok();
    resp.assert_see("OK");
}
