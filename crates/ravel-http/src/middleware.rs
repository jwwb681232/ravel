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
//! # Usage
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
//!
//! let router = Route::new()
//!     .get("/", handler)
//!     .middleware(auth_middleware)
//!     .build();
//! ```

use axum::extract::Request;
use axum::response::Response;

/// Re-export Axum's `Next` type for middleware function signatures.
pub type Next = axum::middleware::Next;

/// Type alias for a Ravel middleware function.
///
/// Any async function `(Request, Next) -> Response` can be used as middleware.
pub type MiddlewareFn = fn(Request, Next) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Response> + Send>,
>;

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
}
