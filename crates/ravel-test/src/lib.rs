//! Test utilities for Ravel applications.
//!
//! Provides helpers for HTTP and application-level testing:
//! - `TestClient` — send requests to your Axum router without a running server
//! - `assert_status` / `assert_see` / `assert_json` — response assertions
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_test::TestClient;
//!
//! let app = my_ravel_app();
//! let client = TestClient::new(app);
//!
//! let resp = client.get("/").await;
//! assert_eq!(resp.status(), 200);
//! assert!(resp.text().contains("Hello"));
//! ```

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::Response;
use serde::de::DeserializeOwned;
use tower::ServiceExt;

/// A test client for sending HTTP requests to an Axum [`Router`].
pub struct TestClient {
    router: Router,
}

impl TestClient {
    /// Create a test client wrapping the given router.
    pub fn new(router: Router) -> Self {
        Self { router }
    }

    /// Send a GET request to `uri`.
    pub async fn get(&self, uri: &str) -> TestResponse {
        self.send(
            Request::builder()
                .uri(uri)
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
    }

    /// Send a POST request with a JSON body.
    pub async fn post_json(&self, uri: &str, body: &str) -> TestResponse {
        self.send(
            Request::builder()
                .uri(uri)
                .method("POST")
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
    }

    /// Send a PUT request with a JSON body.
    pub async fn put_json(&self, uri: &str, body: &str) -> TestResponse {
        self.send(
            Request::builder()
                .uri(uri)
                .method("PUT")
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
    }

    /// Send a PATCH request with a JSON body.
    pub async fn patch_json(&self, uri: &str, body: &str) -> TestResponse {
        self.send(
            Request::builder()
                .uri(uri)
                .method("PATCH")
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
    }

    /// Send a DELETE request.
    pub async fn delete(&self, uri: &str) -> TestResponse {
        self.send(
            Request::builder()
                .uri(uri)
                .method("DELETE")
                .body(Body::empty())
                .unwrap(),
        )
        .await
    }

    /// Send a PATCH request.
    pub async fn patch(&self, uri: &str) -> TestResponse {
        self.send(
            Request::builder()
                .uri(uri)
                .method("PATCH")
                .body(Body::empty())
                .unwrap(),
        )
        .await
    }

    /// Send a raw request.
    pub async fn send(&self, request: Request<Body>) -> TestResponse {
        let response = self.router.clone().oneshot(request).await.unwrap();
        TestResponse::new(response).await
    }
}

/// Wrapper around an Axum [`Response`] with convenience assertions.
pub struct TestResponse {
    status: StatusCode,
    body: String,
}

impl TestResponse {
    async fn new(response: Response<Body>) -> Self {
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap_or_default();
        let body = String::from_utf8_lossy(&body_bytes).to_string();
        Self { status, body }
    }

    /// HTTP status code.
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// Response body as a string.
    pub fn text(&self) -> &str {
        &self.body
    }

    /// Parse the body as JSON.
    pub fn json<T: DeserializeOwned>(&self) -> serde_json::Result<T> {
        serde_json::from_str(&self.body)
    }

    /// Assert the response has a specific status.
    pub fn assert_status(&self, expected: StatusCode) {
        assert_eq!(
            self.status, expected,
            "Expected status {}, got {} (body: {})",
            expected, self.status, self.body
        );
    }

    /// Assert the response body contains the given string.
    pub fn assert_see(&self, needle: &str) {
        assert!(
            self.body.contains(needle),
            "Expected body to contain '{}', but body was: {}",
            needle,
            self.body
        );
    }

    /// Assert the response body matches the expected JSON value.
    pub fn assert_json(&self, expected: serde_json::Value) {
        let actual: serde_json::Value = self.json().expect("Response is not valid JSON");
        assert_eq!(actual, expected, "JSON mismatch");
    }

    /// Assert the response status is 200 OK.
    pub fn assert_ok(&self) {
        self.assert_status(StatusCode::OK);
    }

    /// Assert the response status is 201 Created.
    pub fn assert_created(&self) {
        self.assert_status(StatusCode::CREATED);
    }

    /// Assert the response status is 302 Found (redirect).
    pub fn assert_redirect(&self) {
        self.assert_status(StatusCode::FOUND);
    }

    /// Assert the response status is 401 Unauthorized.
    pub fn assert_unauthorized(&self) {
        self.assert_status(StatusCode::UNAUTHORIZED);
    }

    /// Assert the response status is 403 Forbidden.
    pub fn assert_forbidden(&self) {
        self.assert_status(StatusCode::FORBIDDEN);
    }

    /// Assert the response status is 404 Not Found.
    pub fn assert_not_found(&self) {
        self.assert_status(StatusCode::NOT_FOUND);
    }

    /// Assert the response status is 422 Unprocessable Entity.
    pub fn assert_unprocessable(&self) {
        self.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }

    /// Assert the response body does NOT contain the given string.
    pub fn assert_dont_see(&self, needle: &str) {
        assert!(
            !self.body.contains(needle),
            "Expected body NOT to contain '{}', but body was: {}",
            needle,
            self.body
        );
    }

    /// Assert the response body is exactly the given string.
    pub fn assert_exact(&self, expected: &str) {
        assert_eq!(
            self.body, expected,
            "Expected body to be '{}', but was '{}'",
            expected, self.body
        );
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, routing::get};
    use serde::Serialize;

    async fn hello() -> &'static str {
        "Hello, Ravel!"
    }

    #[derive(Serialize)]
    struct Health {
        status: &'static str,
    }

    async fn health() -> Json<Health> {
        Json(Health { status: "ok" })
    }

    async fn login() -> (StatusCode, Json<serde_json::Value>) {
        (
            StatusCode::CREATED,
            Json(serde_json::json!({"token": "abc"})),
        )
    }

    fn test_app() -> Router {
        Router::new()
            .route("/", get(hello))
            .route("/health", get(health))
            .route("/login", get(login))
    }

    #[tokio::test]
    async fn test_client_get() {
        let client = TestClient::new(test_app());
        let resp = client.get("/").await;
        resp.assert_ok();
        resp.assert_see("Hello");
    }

    #[tokio::test]
    async fn test_client_json() {
        let client = TestClient::new(test_app());
        let resp = client.get("/health").await;
        resp.assert_ok();
        resp.assert_json(serde_json::json!({"status": "ok"}));
    }

    #[tokio::test]
    async fn test_client_created() {
        let client = TestClient::new(test_app());
        let resp = client.get("/login").await;
        resp.assert_created();
        let token: String = resp.json::<serde_json::Value>().unwrap()["token"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(token, "abc");
    }
}
