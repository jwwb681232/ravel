//! Request lifecycle hooks and middleware.
//!
//! Provides:
//! - [`request_id`] — injects X-Request-Id into every request/response
//! - [`request_timer`] — logs request duration
//! - [`LifecycleHooks`] — before/after request hooks
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_http::lifecycle;
//!
//! Route::new()
//!     .middleware(lifecycle::request_id)
//!     .middleware(lifecycle::request_timer)
//!     .build();
//! ```

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;
use std::time::Instant;
use tracing::info;
use uuid::Uuid;

/// Middleware that injects an X-Request-Id header.
///
/// If the client sends an X-Request-Id, it is propagated. Otherwise a
/// new UUIDv4 is generated. The id is available in handlers via
/// [`RequestId`].
pub async fn request_id(mut req: Request, next: Next) -> Response {
    let id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    req.extensions_mut().insert(RequestId(id.clone()));

    let mut resp = next.run(req).await;
    if let Ok(v) = id.parse() {
        resp.headers_mut().insert("x-request-id", v);
    }
    resp
}

/// Middleware that logs request method, path, status, and duration.
pub async fn request_timer(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let start = Instant::now();

    let resp = next.run(req).await;

    let duration = start.elapsed();
    let status = resp.status();
    info!("{method} {path} → {status} ({:.2?})", duration);
    resp
}

// ── RequestId extractor ─────────────────────────────────────────────────

/// Extractor for the current request's unique ID.
///
/// Requires the [`request_id`] middleware to be applied first.
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

impl RequestId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<S: Send + Sync + 'static> axum::extract::FromRequestParts<S> for RequestId {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let id = parts
            .extensions
            .get::<RequestId>()
            .cloned()
            .unwrap_or_else(|| RequestId(Uuid::new_v4().to_string()));
        Ok(id)
    }
}

// ── LifecycleHooks ──────────────────────────────────────────────────────

/// Before/after hooks for request processing.
///
/// Register closures that run on every request:
///
/// ```rust,ignore
/// use ravel_http::lifecycle::LifecycleHooks;
///
/// let mut hooks = LifecycleHooks::new();
/// hooks.before(|req| {
///     tracing::info!("Processing: {}", req.uri().path());
/// });
/// hooks.after(|_req, resp| {
///     tracing::info!("Completed: {}", resp.status());
/// });
/// ```
type BeforeHook = dyn Fn(&Request) + Send + Sync;
type AfterHook = dyn Fn(&Request, &Response) + Send + Sync;

pub struct LifecycleHooks {
    before_hooks: Vec<Arc<BeforeHook>>,
    after_hooks: Vec<Arc<AfterHook>>,
}

impl LifecycleHooks {
    pub fn new() -> Self {
        Self {
            before_hooks: Vec::new(),
            after_hooks: Vec::new(),
        }
    }

    /// Register a hook that runs before the handler.
    pub fn before<F: Fn(&Request) + Send + Sync + 'static>(&mut self, f: F) {
        self.before_hooks.push(Arc::new(f));
    }

    /// Register a hook that runs after the handler.
    pub fn after<F: Fn(&Request, &Response) + Send + Sync + 'static>(&mut self, f: F) {
        self.after_hooks.push(Arc::new(f));
    }

    /// Create an Axum middleware from these hooks.
    pub fn middleware(
        self,
    ) -> impl Fn(
        Request,
        Next,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
    + Clone
    + Send
    + Sync
    + 'static {
        let hooks = Arc::new(self);
        move |req: Request, next: Next| {
            let hooks = hooks.clone();
            Box::pin(async move {
                for hook in &hooks.before_hooks {
                    hook(&req);
                }
                // Capture metadata before req is consumed by next.run()
                let method = req.method().clone();
                let uri = req.uri().clone();
                let headers = req.headers().clone();
                let resp = next.run(req).await;
                for hook in &hooks.after_hooks {
                    let mut snapshot = Request::builder()
                        .method(&method)
                        .uri(&uri)
                        .body(axum::body::Body::empty())
                        .unwrap();
                    *snapshot.headers_mut() = headers.clone();
                    hook(&snapshot, &resp);
                }
                resp
            })
        }
    }
}

impl Default for LifecycleHooks {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_request_id_create() {
        let id = RequestId("abc-123".into());
        assert_eq!(id.as_str(), "abc-123");
    }

    #[test]
    fn test_lifecycle_hooks() {
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let mut hooks = LifecycleHooks::new();
        hooks.before(move |_req| {
            c.fetch_add(1, Ordering::SeqCst);
        });

        // Run the hook
        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        for hook in &hooks.before_hooks {
            hook(&req);
        }
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_lifecycle_hooks_default() {
        let hooks = LifecycleHooks::default();
        assert!(hooks.before_hooks.is_empty());
        assert!(hooks.after_hooks.is_empty());
    }
}
