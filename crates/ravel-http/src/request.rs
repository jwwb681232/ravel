//! Request wrapper — convenience methods on top of `axum::http::Request`.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_http::request::RavelRequest;
//!
//! async fn handler(req: RavelRequest) -> impl IntoResponse {
//!     let name: Option<String> = req.input("name");
//!     let page: u32 = req.query("page").unwrap_or(1);
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
/// - `input(key)` — get a POST/PUT body field (JSON or form)
/// - `query(key)` — get a query-string parameter
/// - `header(key)` — get a request header
/// - `method()` — HTTP method as &str
/// - `path()` — URI path
///
/// You can inject it as an Axum extractor thanks to [`FromRequest`].
pub struct RavelRequest {
    inner: Request<Body>,
    /// Cached parsed query string.
    query_params: Option<HashMap<String, String>>,
}

impl RavelRequest {
    /// Create from the raw Axum request.
    pub fn new(req: Request<Body>) -> Self {
        Self {
            inner: req,
            query_params: None,
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
    /// ```
    pub fn query<T: DeserializeOwned>(&mut self, key: &str) -> Option<T> {
        if self.query_params.is_none() {
            self.query_params = self
                .inner
                .uri()
                .query()
                .map(|qs| {
                    url::form_urlencoded::parse(qs.as_bytes())
                        .into_owned()
                        .collect()
                });
        }

        self.query_params
            .as_ref()
            .and_then(|m| m.get(key))
            .and_then(|v| serde_json::from_str(v).ok())
    }

    /// Check whether a query parameter exists.
    pub fn has_query(&mut self, key: &str) -> bool {
        if self.query_params.is_none() {
            self.query_params = self
                .inner
                .uri()
                .query()
                .map(|qs| {
                    url::form_urlencoded::parse(qs.as_bytes())
                        .into_owned()
                        .collect()
                });
        }
        self.query_params
            .as_ref()
            .map_or(false, |m| m.contains_key(key))
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
            .map_or(false, |v| v.contains("application/json"))
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
