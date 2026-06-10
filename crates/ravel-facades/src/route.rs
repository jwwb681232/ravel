//! Route facade — static route builder (Laravel-style).
//! Use in ServiceProvider::register/boot. Call Route::build() at the end.

use axum::Router;
use axum::routing;
use parking_lot::Mutex;
use std::sync::OnceLock;

static REGISTRY: OnceLock<Mutex<Option<RouteState>>> = OnceLock::new();

/// Collected route metadata for introspection (e.g. `ravel route:list`).
static ROUTE_LIST: OnceLock<Mutex<Vec<RouteEntry>>> = OnceLock::new();

fn route_list() -> &'static Mutex<Vec<RouteEntry>> {
    ROUTE_LIST.get_or_init(|| Mutex::new(Vec::new()))
}

/// A single registered route entry.
#[derive(Debug, Clone)]
pub struct RouteEntry {
    pub method: String,
    pub path: String,
}

struct RouteState {
    router: Router,
    group_stack: Vec<String>,
}

fn registry() -> &'static Mutex<Option<RouteState>> {
    REGISTRY.get_or_init(|| {
        Mutex::new(Some(RouteState {
            router: Router::new(),
            group_stack: Vec::new(),
        }))
    })
}

pub struct Route;

impl Route {
    fn with_router<F>(f: F)
    where
        F: FnOnce(&mut RouteState),
    {
        let mut guard = registry().lock();
        let state = guard.as_mut().expect("Route::build() already called");
        f(state);
    }

    fn full_path(state: &RouteState, path: &str) -> String {
        if state.group_stack.is_empty() {
            path.to_string()
        } else {
            format!("{}{}", state.group_stack.join(""), path)
        }
    }

    /// Record a route entry for introspection.
    fn record(method: &'static str, path: &str) {
        route_list().lock().push(RouteEntry {
            method: method.to_string(),
            path: path.to_string(),
        });
    }

    /// Return all registered routes.
    pub fn list() -> Vec<RouteEntry> {
        route_list().lock().clone()
    }

    /// Clear the route list (done automatically by `reset()`).
    fn clear_list() {
        route_list().lock().clear();
    }

    pub fn get<H, T>(path: &str, handler: H)
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        Self::with_router(|state| {
            let full = Self::full_path(state, path);
            Self::record("GET", &full);
            state.router = state.router.clone().route(&full, routing::get(handler));
        });
    }

    pub fn post<H, T>(path: &str, handler: H)
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        Self::with_router(|state| {
            let full = Self::full_path(state, path);
            Self::record("POST", &full);
            state.router = state.router.clone().route(&full, routing::post(handler));
        });
    }

    pub fn put<H, T>(path: &str, handler: H)
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        Self::with_router(|state| {
            let full = Self::full_path(state, path);
            Self::record("PUT", &full);
            state.router = state.router.clone().route(&full, routing::put(handler));
        });
    }

    pub fn delete<H, T>(path: &str, handler: H)
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        Self::with_router(|state| {
            let full = Self::full_path(state, path);
            Self::record("DELETE", &full);
            state.router = state.router.clone().route(&full, routing::delete(handler));
        });
    }

    pub fn patch<H, T>(path: &str, handler: H)
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        Self::with_router(|state| {
            let full = Self::full_path(state, path);
            Self::record("PATCH", &full);
            state.router = state.router.clone().route(&full, routing::patch(handler));
        });
    }

    pub fn group(prefix: &str, f: impl FnOnce()) {
        Self::with_router(|state| {
            state.group_stack.push(prefix.to_string());
        });
        struct GroupGuard;
        impl Drop for GroupGuard {
            fn drop(&mut self) {
                Route::with_router(|state| {
                    state.group_stack.pop();
                });
            }
        }
        let _guard = GroupGuard;
        f();
    }

    pub fn middleware<F>(f: F)
    where
        F: Fn(
                axum::extract::Request,
                axum::middleware::Next,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = axum::response::Response> + Send>,
            > + Clone
            + Send
            + Sync
            + 'static,
    {
        Self::with_router(|state| {
            state.router = state.router.clone().layer(axum::middleware::from_fn(f));
        });
    }

    pub fn build() -> Router {
        let mut guard = registry().lock();
        let state = guard.as_mut().expect("Route registry not available");
        let router = state.router.clone();
        state.router = Router::new();
        state.group_stack.clear();
        router
    }

    pub fn reset() {
        let mut guard = registry().lock();
        if let Some(state) = guard.as_mut() {
            state.router = Router::new();
            state.group_stack.clear();
        }
        Self::clear_list();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Global state shared across Route facade tests — serialise them.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    async fn dummy_handler() -> &'static str {
        "ok"
    }

    #[test]
    fn test_route_list_records_routes() {
        let _lock = TEST_LOCK.lock().unwrap();
        Route::reset();
        Route::get("/", dummy_handler);
        Route::post("/users", dummy_handler);
        Route::delete("/users/{id}", dummy_handler);

        let routes = Route::list();
        assert_eq!(routes.len(), 3);
        assert_eq!(routes[0].method, "GET");
        assert_eq!(routes[0].path, "/");
        assert_eq!(routes[1].method, "POST");
        assert_eq!(routes[1].path, "/users");
        assert_eq!(routes[2].method, "DELETE");
        assert_eq!(routes[2].path, "/users/{id}");
    }

    #[test]
    fn test_route_list_respects_group_prefix() {
        let _lock = TEST_LOCK.lock().unwrap();
        Route::reset();
        Route::group("/api", || {
            Route::get("/health", dummy_handler);
            Route::post("/login", dummy_handler);
        });

        let routes = Route::list();
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].path, "/api/health");
        assert_eq!(routes[1].path, "/api/login");
    }

    #[test]
    fn test_route_list_resets() {
        let _lock = TEST_LOCK.lock().unwrap();
        Route::reset();
        Route::get("/a", dummy_handler);
        assert_eq!(Route::list().len(), 1);

        Route::reset();
        assert!(Route::list().is_empty());

        Route::get("/b", dummy_handler);
        assert_eq!(Route::list().len(), 1);
    }
}
