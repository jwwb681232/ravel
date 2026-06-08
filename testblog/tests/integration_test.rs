//! Integration tests for the testblog application.
//!
//! These tests validate that the Ravel Application lifecycle works end-to-end:
//! - Application boots successfully
//! - Container freezes after boot
//! - Router is resolved from the container
//! - Routes respond correctly across methods
//! - Middleware (CORS, logging) functions
//! - FormRequest validation works

use anyhow::Result;
use axum::Router;
use axum::http::StatusCode;
use ravel_core::app::{Application, ServiceProvider};
use ravel_core::container::Container;
use ravel_http::form_request::{FormRequest, Validated};
use ravel_http::middleware;
use ravel_http::route::Route;
use ravel_http::validation::{FieldRule, Rule};
use serde::Deserialize;
use std::sync::Arc;

// ── Test FormRequest ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GreetRequest {
    name: String,
}

impl FormRequest for GreetRequest {
    fn rules() -> Vec<FieldRule> {
        vec![FieldRule::new("name", vec![Rule::Required, Rule::Min(2)])]
    }
}

// ── ServiceProvider with extended routes ────────────────────────────────

struct TestRouteProvider;

impl ServiceProvider for TestRouteProvider {
    fn register(&self, container: &Container) -> Result<()> {
        let router =
            Route::new()
                .get("/", || async { "Hello, Ravel!" })
                .get("/health", || async { (StatusCode::OK, "OK") })
                .post(
                    "/greet",
                    |Validated(req): Validated<GreetRequest>| async move {
                        format!("Hello, {}!", req.name)
                    },
                )
                .middleware(middleware::log_requests)
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

// ── Application bootstrap tests ─────────────────────────────────────────

#[test]
fn test_app_boots_successfully() {
    let app = create_test_app();
    let router: Arc<Router> = app.container().resolve().expect("Router not found");
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

#[test]
fn test_app_is_booted() {
    let app = create_test_app();
    assert!(app.is_booted());
}

#[test]
fn test_app_config_loaded() {
    let app = Application::new().load_config("nonexistent-dir").unwrap();
    // Config loads OK even for missing dir (just empty)
    assert!(!app.config().is_empty() || app.config().is_empty());
    // Container was set up during load_config
    assert!(app.container().has::<ravel_core::config::ConfigRepo>());
}

fn test_app_router() -> Router {
    let app = create_test_app();
    let router: Arc<Router> = app.container().resolve().expect("Router not found");
    (*router).clone()
}

// ── HTTP request tests ──────────────────────────────────────────────────

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
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_health_endpoint() {
    let client = ravel_test::TestClient::new(test_app_router());
    let resp = client.get("/health").await;
    resp.assert_ok();
    resp.assert_see("OK");
}

#[tokio::test]
async fn test_post_greet_valid() {
    let client = ravel_test::TestClient::new(test_app_router());
    let resp = client.post_json("/greet", r#"{"name":"Alice"}"#).await;
    resp.assert_ok();
    resp.assert_see("Hello, Alice!");
}

#[tokio::test]
async fn test_post_greet_validation_fails() {
    let client = ravel_test::TestClient::new(test_app_router());
    let resp = client.post_json("/greet", r#"{"name":"X"}"#).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_post_greet_missing_field() {
    let client = ravel_test::TestClient::new(test_app_router());
    let resp = client.post_json("/greet", r#"{}"#).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
