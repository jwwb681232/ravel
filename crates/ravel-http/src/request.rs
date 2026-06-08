//! Request wrapper — convenience methods on top of `axum::http::Request`.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_http::request::RavelRequest;
//!
//! async fn handler(req: RavelRequest) -> impl IntoResponse {
//!     let page: u32 = req.query("page").unwrap_or(1);
//!     let name: Option<String> = req.query("name");
//!     // ...
//! }
//! ```

use axum::body::Body;
use axum::extract::{FromRequest, Request};
use axum::http::HeaderMap;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

/// A Ravel-flavoured request wrapper.
///
/// Wraps an [`axum::extract::Request`] and provides Laravel-style helpers:
/// - `query(key)` — get a query-string parameter
/// - `header(key)` — get a request header
/// - `method()` — HTTP method as &str
/// - `path()` — URI path
///
/// Query parameters are parsed eagerly on construction (cheap — each request
/// gets a fresh `RavelRequest`), so all accessors use `&self`.
///
/// You can inject it as an Axum extractor thanks to [`FromRequest`].
pub struct RavelRequest {
    inner: Request<Body>,
    query_params: HashMap<String, String>,
}

impl RavelRequest {
    /// Create from the raw Axum request.
    pub fn new(req: Request<Body>) -> Self {
        let query_params = req
            .uri()
            .query()
            .map(|qs| {
                url::form_urlencoded::parse(qs.as_bytes())
                    .into_owned()
                    .collect()
            })
            .unwrap_or_default();

        Self {
            inner: req,
            query_params,
        }
    }

    /// Create a RavelRequest from a reference to an Axum request (without consuming it).
    /// Used by middleware to capture request metadata for facades.
    pub fn from_request_ref(req: &Request<Body>) -> Self {
        let query_params = req
            .uri()
            .query()
            .map(|qs| {
                url::form_urlencoded::parse(qs.as_bytes())
                    .into_owned()
                    .collect()
            })
            .unwrap_or_default();

        // Clone request metadata (method, uri, headers) without consuming the body.
        let mut cloned = Request::new(Body::empty());
        *cloned.uri_mut() = req.uri().clone();
        *cloned.method_mut() = req.method().clone();
        *cloned.headers_mut() = req.headers().clone();

        Self {
            inner: cloned,
            query_params,
        }
    }

    /// Return the HTTP method (e.g. `"GET"`, `"POST"`).
    pub fn method(&self) -> &str {
        self.inner.method().as_str()
    }

    /// Return the URI path (e.g. `"/users/42"`).
    pub fn path(&self) -> &str {
        self.inner.uri().path()
    }

    /// Return the full query string, if any.
    pub fn query_string(&self) -> Option<&str> {
        self.inner.uri().query()
    }

    /// Get a query-string parameter by name.
    ///
    /// ```rust,ignore
    /// let page: u32 = req.query("page").unwrap_or(1);
    /// let name: String = req.query("name").unwrap_or_default();
    /// ```
    pub fn query<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let raw = self.query_params.get(key)?;
        // Try direct JSON parse (works for numbers, booleans, etc.)
        // Fall back to string-wrapped parse (works for raw string values)
        serde_json::from_str(raw)
            .or_else(|_| serde_json::from_str(&format!("\"{}\"", raw)))
            .ok()
    }

    /// Check whether a query parameter exists.
    pub fn has_query(&self, key: &str) -> bool {
        self.query_params.contains_key(key)
    }

    /// Get all query parameters as a map.
    pub fn queries(&self) -> &HashMap<String, String> {
        &self.query_params
    }

    /// Get a specific header value.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.inner.headers().get(name)?.to_str().ok()
    }

    /// Get all headers.
    pub fn headers(&self) -> &HeaderMap {
        self.inner.headers()
    }

    /// Get the `Content-Type` header.
    pub fn content_type(&self) -> Option<&str> {
        self.header("content-type")
    }

    /// Check whether the request expects a JSON response.
    pub fn wants_json(&self) -> bool {
        self.header("accept")
            .is_some_and(|v| v.contains("application/json"))
    }

    /// Return the raw inner Axum request (consumes self).
    pub fn into_inner(self) -> Request<Body> {
        self.inner
    }

    /// Return a reference to the raw Axum request.
    pub fn inner(&self) -> &Request<Body> {
        &self.inner
    }
}

// ── FromRequest impl — allows RavelRequest as an extractor ─────────

impl<S: Send + Sync + 'static> FromRequest<S> for RavelRequest {
    type Rejection = std::convert::Infallible;

    async fn from_request(req: Request<Body>, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self::new(req))
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_self() {
        let req = Request::builder()
            .uri("/test?name=alice&age=30")
            .body(Body::empty())
            .unwrap();

        let r: &RavelRequest = &RavelRequest::new(req);
        // Both query() and has_query() should work with &self
        assert_eq!(r.query::<String>("name").unwrap(), "alice");
        assert_eq!(r.query::<i32>("age").unwrap(), 30);
        assert!(r.has_query("name"));
        assert!(!r.has_query("missing"));
        assert!(r.query::<String>("missing").is_none());
    }

    #[test]
    fn test_empty_query() {
        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();

        let r = RavelRequest::new(req);
        assert!(r.queries().is_empty());
        assert!(!r.has_query("anything"));
    }
}
