//! Request-level facade support -- task-local context for Auth, Session, request().
#![allow(clippy::arc_with_non_send_sync)]

use crate::request::RavelRequest;
use crate::session::SessionState;
use axum::extract::Request as AxumRequest;
use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;

// -- Request Context ---------------------------------------------------

/// Per-request state accessible to facades via [`REQUEST`].
///
/// Both the session and auth facades read/write the shared
/// [`SessionState`] that was created by [`SessionService`].
/// This means `ravel_facades::Session::put("key", val)` and
/// `ravel_http::session::Session::get::<T>("key")` see the same data.
pub struct RequestContext {
    /// The raw HTTP request wrapper.
    pub request: RavelRequest,
    /// Shared session state — same [`Arc`] held by [`Session`] extractor.
    pub session: Arc<SessionState>,
}

impl RequestContext {
    /// Extract context from request extensions.
    ///
    /// Must be called AFTER [`SessionService`] has inserted the shared
    /// [`SessionState`] into extensions.
    pub fn from_extensions(req: &AxumRequest) -> Self {
        let request = RavelRequest::from_request_ref(req);
        let session = req
            .extensions()
            .get::<Arc<SessionState>>()
            .cloned()
            .unwrap_or_default();
        Self { request, session }
    }
}

// -- Task-Local --------------------------------------------------------

tokio::task_local! {
    /// Per-request context, set by [`start_request`] middleware.
    /// Accessible via `REQUEST.try_with(|ctx| ...)` inside HTTP handlers.
    pub static REQUEST: Arc<RequestContext>;
}

// -- Middleware --------------------------------------------------------

/// Axum middleware that injects [`RequestContext`] into [`REQUEST`].
///
/// This should run AFTER [`SessionService`] so the shared
/// [`SessionState`] is already available in request extensions.
pub async fn start_request(req: AxumRequest, next: Next) -> Response {
    let ctx = Arc::new(RequestContext::from_extensions(&req));
    REQUEST.scope(ctx, next.run(req)).await
}
