//! Route facade — static route builder (Laravel-style).
//! Use in ServiceProvider::register/boot. Call Route::build() at the end.

use axum::Router;
use axum::routing;
use parking_lot::Mutex;
use std::sync::OnceLock;

static REGISTRY: OnceLock<Mutex<Option<RouteState>>> = OnceLock::new();

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

    pub fn get(
        path: &str,
        handler: impl axum::handler::Handler<(), ()> + Clone + Send + Sync + 'static,
    ) {
        Self::with_router(|state| {
            let full = Self::full_path(state, path);
            state.router = state.router.clone().route(&full, routing::get(handler));
        });
    }

    pub fn post(
        path: &str,
        handler: impl axum::handler::Handler<(), ()> + Clone + Send + Sync + 'static,
    ) {
        Self::with_router(|state| {
            let full = Self::full_path(state, path);
            state.router = state.router.clone().route(&full, routing::post(handler));
        });
    }

    pub fn put(
        path: &str,
        handler: impl axum::handler::Handler<(), ()> + Clone + Send + Sync + 'static,
    ) {
        Self::with_router(|state| {
            let full = Self::full_path(state, path);
            state.router = state.router.clone().route(&full, routing::put(handler));
        });
    }

    pub fn delete(
        path: &str,
        handler: impl axum::handler::Handler<(), ()> + Clone + Send + Sync + 'static,
    ) {
        Self::with_router(|state| {
            let full = Self::full_path(state, path);
            state.router = state.router.clone().route(&full, routing::delete(handler));
        });
    }

    pub fn patch(
        path: &str,
        handler: impl axum::handler::Handler<(), ()> + Clone + Send + Sync + 'static,
    ) {
        Self::with_router(|state| {
            let full = Self::full_path(state, path);
            state.router = state.router.clone().route(&full, routing::patch(handler));
        });
    }

    pub fn group(prefix: &str, f: impl FnOnce()) {
        Self::with_router(|state| {
            state.group_stack.push(prefix.to_string());
        });
        f();
        Self::with_router(|state| {
            state.group_stack.pop();
        });
    }

    pub fn middleware<F>(f: F)
    where
        F: Fn(axum::extract::Request, axum::middleware::Next) -> std::pin::Pin<
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

    /// Build the final Router. Consumes the registry.
    /// Subsequent calls to Route methods will panic.
    pub fn build() -> Router {
        let mut guard = registry().lock();
        guard.take().expect("Route::build() already called").router
    }
}
