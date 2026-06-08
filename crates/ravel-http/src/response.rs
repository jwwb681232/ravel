//! Response builder — Laravel-inspired response helpers.
//!
//! Provides:
//! - `response().json(data)` — JSON response
//! - `response().status(200).body("ok")` — custom status + body
//! - `response().redirect("/")` — 302 redirect
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_http::response::ResponseBuilder;
//! use ravel_http::Json;
//!
//! async fn handler() -> impl IntoResponse {
//!     ResponseBuilder::new()
//!         .json(serde_json::json!({"ok": true}))
//!         .unwrap()
//! }
//! ```
//!
//! Note: `ResponseBuilder` implements [`axum::response::IntoResponse`], so
//! you can return it directly from handlers without calling `.build()`.

use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use serde::Serialize;

/// Convenience constructor.
pub fn response() -> ResponseBuilder {
    ResponseBuilder::new()
}

// ── ResponseBuilder ────────────────────────────────────────────────

/// Build an Axum-compatible response with a fluent API.
pub struct ResponseBuilder {
    status: StatusCode,
    headers: HeaderMap,
    body: Option<Response<Body>>,
}

impl ResponseBuilder {
    /// Start building a 200 OK response.
    pub fn new() -> Self {
        Self {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: None,
        }
    }

    /// Set the HTTP status code.
    pub fn status(mut self, code: StatusCode) -> Self {
        self.status = code;
        self
    }

    /// Add a custom header.
    pub fn header(mut self, key: &str, value: &str) -> Self {
        if let (Ok(k), Ok(v)) = (
            HeaderName::from_bytes(key.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            self.headers.insert(k, v);
        }
        self
    }

    /// Set the response body to a plain-text string.
    pub fn body(mut self, text: impl Into<String>) -> Self {
        let text = text.into();
        let mut resp = Response::new(Body::from(text));
        *resp.status_mut() = self.status;
        for (k, v) in &self.headers {
            resp.headers_mut().insert(k.clone(), v.clone());
        }
        self.body = Some(resp);
        self
    }

    /// Set the response body to a JSON value.
    pub fn json<T: Serialize>(mut self, data: T) -> Result<Self, serde_json::Error> {
        let json_body = serde_json::to_string(&data)?;
        let mut resp = Json(data).into_response();
        *resp.status_mut() = self.status;
        for (k, v) in &self.headers {
            resp.headers_mut().insert(k.clone(), v.clone());
        }
        self.body = Some(resp.map(|_| Body::from(json_body)));
        Ok(self)
    }

    /// Return a JSON response with the given status code (shorthand).
    pub fn json_status<T: Serialize>(
        code: StatusCode,
        data: T,
    ) -> Result<Response<Body>, serde_json::Error> {
        Ok(ResponseBuilder::new()
            .status(code)
            .json(data)?
            .into_response())
    }

    /// Build a redirect (302 Found) to `url`.
    pub fn redirect(url: &str) -> Response<Body> {
        let mut resp = Response::new(Body::empty());
        *resp.status_mut() = StatusCode::FOUND;
        resp.headers_mut().insert("Location", url.parse().unwrap());
        resp
    }

    /// Build a permanent redirect (301 Moved Permanently) to `url`.
    pub fn redirect_permanent(url: &str) -> Response<Body> {
        let mut resp = Response::new(Body::empty());
        *resp.status_mut() = StatusCode::MOVED_PERMANENTLY;
        resp.headers_mut().insert("Location", url.parse().unwrap());
        resp
    }

    /// Finalise and produce an Axum [`Response`].
    pub fn build(self) -> Result<Response<Body>, String> {
        self.body.ok_or_else(|| "No body set on response".into())
    }
}

impl Default for ResponseBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoResponse for ResponseBuilder {
    fn into_response(self) -> Response<Body> {
        self.body.unwrap_or_else(|| {
            let mut resp = Response::new(Body::empty());
            *resp.status_mut() = self.status;
            resp
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_response() {
        let resp = ResponseBuilder::new()
            .json(serde_json::json!({"status": "ok"}))
            .unwrap()
            .build()
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn test_custom_status() {
        let resp = ResponseBuilder::new()
            .status(StatusCode::CREATED)
            .body("created")
            .build()
            .unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    #[test]
    fn test_response_builder_into_response() {
        let resp: Response<Body> = ResponseBuilder::new()
            .status(StatusCode::OK)
            .body("hello")
            .into_response();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn test_redirect_is_302() {
        let resp = ResponseBuilder::redirect("/login");
        assert_eq!(resp.status(), StatusCode::FOUND);
    }
}
