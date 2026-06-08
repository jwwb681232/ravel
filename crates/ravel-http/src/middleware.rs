//! Middleware pipeline — Axum-compatible middleware helpers.
//!
//! Ravel middleware is simply an async function with the signature:
//!
//! ```rust,ignore
//! async fn my_middleware(req: Request, next: Next) -> Response {
//!     // pre-processing
//!     let response = next.run(req).await;
//!     // post-processing
//!     response
//! }
//! ```
//!
//! Apply it to routes via [`Route::middleware`](crate::route::Route::middleware).
//!
//! # Built-in middleware
//!
//! - [`error_handler`] — catches panics and returns 500 JSON responses
//! - [`cors`] — adds CORS headers (configurable origins, methods, headers)
//! - [`log_requests`] — logs request method, path, and response status
//!
//! # Custom middleware
//!
//! ```rust,ignore
//! use ravel_http::middleware::Next;
//! use axum::extract::Request;
//! use axum::response::Response;
//!
//! async fn auth_middleware(req: Request, next: Next) -> Response {
//!     // Check auth header...
//!     next.run(req).await
//! }
//! ```

use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tracing::info;

/// Re-export Axum's `Next` type for middleware function signatures.
pub type Next = axum::middleware::Next;

/// Type alias for a Ravel middleware function.
///
/// Any async function `(Request, Next) -> Response` can be used as middleware.
pub type MiddlewareFn = fn(Request, Next) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Response> + Send>,
>;

// ── Built-in middleware ───────────────────────────────────────────

/// Error handling middleware — catches panics and returns 500 JSON.
///
/// ```rust,ignore
/// use ravel_http::middleware::error_handler;
///
/// let router = Route::new()
///     .get("/", handler)
///     .middleware(error_handler)
///     .build();
/// ```
pub async fn error_handler(req: Request, next: Next) -> Response {
    let result = std::panic::AssertUnwindSafe(next.run(req));
    match std::panic::catch_unwind(|| result) {
        Ok(future) => match tokio::task::spawn(future).await {
            Ok(response) => response,
            Err(e) => {
                let msg = if e.is_panic() {
                    "Internal server error (panic)"
                } else {
                    "Internal server error"
                };
                error_response(StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
        },
        Err(_) => error_response(StatusCode::INTERNAL_SERVER_ERROR, "Internal server error (panic)"),
    }
}

/// Request logging middleware — logs method, path, and status.
///
/// ```rust,ignore
/// use ravel_http::middleware::log_requests;
///
/// let router = Route::new()
///     .get("/", handler)
///     .middleware(log_requests)
///     .build();
/// ```
pub async fn log_requests(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    let response = next.run(req).await;
    let status = response.status();

    info!("{method} {path} → {status}");
    response
}

/// CORS configuration builder.
///
/// ```rust,ignore
/// use ravel_http::middleware::CorsConfig;
///
/// let cors = CorsConfig::new()
///     .allow_origin("http://localhost:3000")
///     .allow_methods("GET, POST, PUT, DELETE")
///     .allow_headers("Content-Type, Authorization");
///
/// let router = Route::new()
///     .get("/", handler)
///     .middleware(cors.middleware())
///     .build();
/// ```
#[derive(Clone)]
pub struct CorsConfig {
    allow_origin: String,
    allow_methods: String,
    allow_headers: String,
    max_age: u64,
}

impl CorsConfig {
    pub fn new() -> Self {
        Self {
            allow_origin: "*".into(),
            allow_methods: "GET, POST, PUT, DELETE, PATCH, OPTIONS".into(),
            allow_headers: "Content-Type, Authorization".into(),
            max_age: 3600,
        }
    }

    pub fn allow_origin(mut self, origin: impl Into<String>) -> Self {
        self.allow_origin = origin.into();
        self
    }

    pub fn allow_methods(mut self, methods: impl Into<String>) -> Self {
        self.allow_methods = methods.into();
        self
    }

    pub fn allow_headers(mut self, headers: impl Into<String>) -> Self {
        self.allow_headers = headers.into();
        self
    }

    pub fn max_age(mut self, seconds: u64) -> Self {
        self.max_age = seconds;
        self
    }

    /// Create a middleware function from this CORS config.
    pub fn middleware(self) -> impl Fn(Request, Next) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Response> + Send>,
    > + Clone + Send + Sync + 'static {
        move |req: Request, next: Next| {
            let this = self.clone();
            Box::pin(async move { this.handle(req, next).await })
        }
    }

    async fn handle(&self, req: Request, next: Next) -> Response {
        let is_preflight = req.method() == axum::http::Method::OPTIONS;

        let mut response = if is_preflight {
            StatusCode::NO_CONTENT.into_response()
        } else {
            next.run(req).await
        };

        let headers = response.headers_mut();
        headers.insert(
            "Access-Control-Allow-Origin",
            self.allow_origin.parse().unwrap(),
        );
        headers.insert(
            "Access-Control-Allow-Methods",
            self.allow_methods.parse().unwrap(),
        );
        headers.insert(
            "Access-Control-Allow-Headers",
            self.allow_headers.parse().unwrap(),
        );
        headers.insert(
            "Access-Control-Max-Age",
            self.max_age.to_string().parse().unwrap(),
        );

        response
    }
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper: create a JSON error response.
fn error_response(status: StatusCode, message: &str) -> Response {
    (
        status,
        axum::Json(serde_json::json!({
            "message": message,
            "status": status.as_u16(),
        })),
    )
        .into_response()
}

// ── Auth middleware ───────────────────────────────────────────────

/// Bearer token authentication middleware.
///
/// Checks the `Authorization: Bearer <token>` header and validates
/// the token using a user-provided validator function.
///
/// On success, the validated user ID is stored in request extensions.
///
/// ```rust,ignore
/// use ravel_http::middleware::BearerAuth;
///
/// // Validate token → return Some(user_id) or None
/// let auth = BearerAuth::new(|token: &str| async move {
///     if token == "valid-token" { Some("user-1".to_string()) } else { None }
/// });
///
/// let router = Route::new()
///     .get("/protected", handler)
///     .middleware(auth.middleware())
///     .build();
/// ```
pub struct BearerAuth<F, Fut, UserId> {
    validator: F,
    _phantom: std::marker::PhantomData<fn(Fut) -> UserId>,
}

impl<F: Clone, Fut, UserId> Clone for BearerAuth<F, Fut, UserId> {
    fn clone(&self) -> Self {
        Self {
            validator: self.validator.clone(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<F, Fut, UserId> BearerAuth<F, Fut, UserId>
where
    F: Fn(String) -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = Option<UserId>> + Send + 'static,
    UserId: Clone + Send + Sync + 'static,
{
    pub fn new(validator: F) -> Self {
        Self {
            validator,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Create a middleware function from this auth config.
    pub fn middleware(self) -> impl Fn(Request, Next) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Response> + Send>,
    > + Clone + Send + Sync + 'static {
        move |req: Request, next: Next| {
            let this = self.clone();
            Box::pin(async move { this.handle(req, next).await })
        }
    }

    async fn handle(&self, mut req: Request, next: Next) -> Response {
        // Extract bearer token
        let token = req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|s| s.to_string());

        match token {
            Some(token) => {
                match (self.validator)(token).await {
                    Some(user_id) => {
                        req.extensions_mut().insert(user_id);
                        next.run(req).await
                    }
                    None => error_response(StatusCode::UNAUTHORIZED, "Invalid token"),
                }
            }
            None => error_response(
                StatusCode::UNAUTHORIZED,
                "Missing Authorization header",
            ),
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::Router;
    use axum::routing::get;
    use tower::ServiceExt;

    async fn hello_handler() -> &'static str {
        "Hello!"
    }

    async fn add_header_middleware(req: Request, next: Next) -> Response {
        let mut response = next.run(req).await;
        response
            .headers_mut()
            .insert("X-Custom", "ravel".parse().unwrap());
        response
    }

    #[tokio::test]
    async fn test_middleware_adds_header() {
        let app = Router::new()
            .route("/", get(hello_handler))
            .layer(axum::middleware::from_fn(add_header_middleware));

        let req = Request::builder()
            .uri("/")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("X-Custom").unwrap(),
            "ravel"
        );
    }

    async fn reject_middleware(_req: Request, _next: Next) -> Response {
        StatusCode::FORBIDDEN.into_response()
    }

    #[tokio::test]
    async fn test_middleware_can_short_circuit() {
        let app = Router::new()
            .route("/", get(hello_handler))
            .layer(axum::middleware::from_fn(reject_middleware));

        let req = Request::builder()
            .uri("/")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_cors_adds_headers() {
        let cors = CorsConfig::new()
            .allow_origin("http://localhost:3000");

        let app = Router::new()
            .route("/", get(hello_handler))
            .layer(axum::middleware::from_fn(cors.middleware()));

        let req = Request::builder()
            .uri("/")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("Access-Control-Allow-Origin").unwrap(),
            "http://localhost:3000"
        );
    }

    #[tokio::test]
    async fn test_cors_preflight() {
        let cors = CorsConfig::new();

        let app = Router::new()
            .route("/", get(hello_handler))
            .layer(axum::middleware::from_fn(cors.middleware()));

        let req = Request::builder()
            .uri("/")
            .method("OPTIONS")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(response.headers().contains_key("Access-Control-Allow-Methods"));
    }

    #[tokio::test]
    async fn test_log_requests_passes_through() {
        let app = Router::new()
            .route("/", get(hello_handler))
            .layer(axum::middleware::from_fn(log_requests));

        let req = Request::builder()
            .uri("/")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_bearer_auth_valid_token() {
        let auth = BearerAuth::new(|token: String| async move {
            if token == "valid-token" {
                Some("user-1".to_string())
            } else {
                None
            }
        });

        let app = Router::new()
            .route("/", get(hello_handler))
            .layer(axum::middleware::from_fn(auth.middleware()));

        let req = Request::builder()
            .uri("/")
            .header("authorization", "Bearer valid-token")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_bearer_auth_invalid_token() {
        let auth = BearerAuth::new(|_token: String| async move { None::<String> });

        let app = Router::new()
            .route("/", get(hello_handler))
            .layer(axum::middleware::from_fn(auth.middleware()));

        let req = Request::builder()
            .uri("/")
            .header("authorization", "Bearer bad-token")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_bearer_auth_missing_header() {
        let auth = BearerAuth::new(|_token: String| async move { None::<String> });

        let app = Router::new()
            .route("/", get(hello_handler))
            .layer(axum::middleware::from_fn(auth.middleware()));

        let req = Request::builder()
            .uri("/")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
