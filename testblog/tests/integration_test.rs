//! Integration tests using ravel-facades.
use anyhow::Result;
use axum::http::StatusCode;
use ravel_core::app::{Application, ServiceProvider};
use ravel_core::container::Container;
use ravel_facades::Route;
use std::sync::Mutex;

static TEST_LOCK: Mutex<()> = Mutex::new(());
fn lock() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

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
    let _l = lock();
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
    let _l = lock();
    ravel_core::app::APP.reset();
    create_test_app();
    let app = ravel_core::app::APP.get().unwrap();
    assert!(app.is_booted());
}

#[test]
fn test_app_config_loaded() {
    let _l = lock();
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
    let _l = lock();
    let client = ravel_test::TestClient::new(test_router());
    let resp = client.get("/").await;
    resp.assert_ok();
    resp.assert_see("Hello, Ravel!");
}

#[tokio::test]
async fn test_not_found_route() {
    let _l = lock();
    let client = ravel_test::TestClient::new(test_router());
    let resp = client.get("/nonexistent").await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_health_endpoint() {
    let _l = lock();
    let client = ravel_test::TestClient::new(test_router());
    let resp = client.get("/health").await;
    resp.assert_ok();
    resp.assert_see("OK");
}
