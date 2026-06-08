//! Route DSL — a Laravel-inspired ergonomic API on top of Axum.
//!
//! # Quick start
//!
//! ```rust,ignore
//! use ravel_http::route::Route;
//! use axum::{Router, response::IntoResponse};
//!
//! async fn index() -> impl IntoResponse { "Hello" }
//!
//! let app: Router = Route::new()
//!     .get("/", index)
//!     .get("/hello/:name", |name: Path<String>| async move { ... })
//!     .group("/api", |api| {
//!         api.get("/users", list_users)
//!            .post("/users", create_user);
//!     })
//!     .build();
//! ```

use axum::routing;
use axum::Router;

// Re-export useful axum types for convenience.
pub use axum::extract::{Path, Query, State};
pub use axum::Json;

// ── Route ──────────────────────────────────────────────────────────

/// The top-level route builder.
///
/// Collects route definitions and finally materialises to an
/// [`axum::Router`] via [`build`](Self::build) or [`into_router`](Self::into_router).
pub struct Route {
    router: Router,
}

impl Route {
    /// Create a new, empty route set.
    pub fn new() -> Self {
        Self {
            router: Router::new(),
        }
    }

    /// Register a handler for `GET <path>`.
    ///
    /// The handler can be any async function whose return type implements
    /// [`IntoResponse`].
    pub fn get<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        self.router = self.router.route(path, routing::get(handler));
        self
    }

    /// Register a handler for `POST <path>`.
    pub fn post<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        self.router = self.router.route(path, routing::post(handler));
        self
    }

    /// Register a handler for `PUT <path>`.
    pub fn put<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        self.router = self.router.route(path, routing::put(handler));
        self
    }

    /// Register a handler for `DELETE <path>`.
    pub fn delete<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        self.router = self.router.route(path, routing::delete(handler));
        self
    }

    /// Register a handler for `PATCH <path>`.
    pub fn patch<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        self.router = self.router.route(path, routing::patch(handler));
        self
    }

    /// Merge a sub-`Router` into the current route set at `prefix`.
    ///
    /// This is the low-level version of [`group`](Self::group).  Use
    /// `group` when you want the builder pattern; use `merge` when you
    /// already have a `Router` or a `Route` built separately.
    pub fn merge(mut self, prefix: &str, router: Router) -> Self {
        self.router = self.router.nest(prefix, router);
        self
    }

    /// Define a **route group** with a shared prefix.
    ///
    /// The closure receives a fresh `Route` builder whose paths are
    /// relative to `prefix`.
    ///
    /// ```rust,ignore
    /// Route::new()
    ///     .group("/admin", |admin| {
    ///         admin.get("/dashboard", dashboard)
    ///              .get("/users", list_users);
    ///     })
    ///     .build();
    /// ```
    pub fn group(
        mut self,
        prefix: &str,
        f: impl FnOnce(Route) -> Route,
    ) -> Self {
        let group_routes = f(Route::new());
        self.router = self.router.nest(prefix, group_routes.into_router());
        self
    }

    /// Apply a function that transforms the inner Router.
    ///
    /// This is the escape hatch for applying middleware, additional state,
    /// or any other axum::Router method that isn't directly exposed.
    ///
    /// ```rust,ignore
    /// use tower_http::cors::CorsLayer;
    ///
    /// Route::new()
    ///     .get("/", handler)
    ///     .with(|r| r.layer(CorsLayer::permissive()))
    ///     .build();
    /// ```
    pub fn with(mut self, f: impl FnOnce(Router) -> Router) -> Self {
        self.router = f(self.router);
        self
    }

    /// Materialise the route definitions into an [`axum::Router`].
    pub fn build(self) -> Router {
        self.router
    }

    /// Consume `self` and return the inner [`axum::Router`].
    pub fn into_router(self) -> Router {
        self.router
    }

    /// Return a reference to the inner [`axum::Router`].
    pub fn inner(&self) -> &Router {
        &self.router
    }
}

impl Default for Route {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests (compile-time only for now) ─────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    async fn index() -> &'static str {
        "Hello, Ravel!"
    }

    async fn user_show(Path(id): Path<String>) -> String {
        format!("User {id}")
    }

    async fn user_create() -> impl IntoResponse {
        (StatusCode::CREATED, "created")
    }

    #[test]
    fn test_basic_routes_compile() {
        let router = Route::new()
            .get("/", index)
            .get("/users/{id}", user_show)
            .post("/users", user_create)
            .put("/users/{id}", user_show)
            .delete("/users/{id}", user_show)
            .patch("/users/{id}", user_show)
            .build();

        // Just ensure it compiles and produces a Router
        let _ = router;
    }

    #[test]
    fn test_route_group() {
        let router = Route::new()
            .get("/", index)
            .group("/api", |api| {
                api.get("/status", index)
                    .post("/users", user_create)
            })
            .build();

        let _ = router;
    }

    #[test]
    fn test_merge_sub_router() {
        let sub = Route::new()
            .get("/ping", index)
            .into_router();

        let router = Route::new()
            .merge("/v1", sub)
            .build();

        let _ = router;
    }

    #[test]
    fn test_default_creates_empty_router() {
        let route = Route::default();
        let _router = route.build();
    }
}
