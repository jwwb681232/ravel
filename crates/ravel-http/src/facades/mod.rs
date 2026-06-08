//! Request-level facade support -- task-local context for Auth, Session, request().

use crate::request::RavelRequest;
use crate::session::SessionData;
use axum::extract::Request as AxumRequest;
use axum::middleware::Next;
use axum::response::Response;
use parking_lot::Mutex;
use std::sync::Arc;

// -- Request Context ---------------------------------------------------

/// Per-request state accessible to facades via [`REQUEST`].
#[allow(dead_code)]
pub struct RequestContext {
    /// The raw HTTP request wrapper.
    pub request: RavelRequest,
    /// Mutable session data (read/write during request).
    pub(crate) session: Mutex<SessionData>,
    /// Current authenticated user ID, if any.
    pub(crate) auth_id: Mutex<Option<String>>,
}

impl RequestContext {
    /// Extract context from an incoming Axum request.
    pub fn from_request(req: &AxumRequest) -> Self {
        let request = RavelRequest::from_request_ref(req);
        Self {
            request,
            session: Mutex::new(SessionData::default()),
            auth_id: Mutex::new(None),
        }
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
/// This should be the outermost middleware so all downstream handlers
/// and middlewares can access Auth/Session/request().
pub async fn start_request(req: AxumRequest, next: Next) -> Response {
    let ctx = Arc::new(RequestContext::from_request(&req));
    REQUEST.scope(ctx, next.run(req)).await
}
