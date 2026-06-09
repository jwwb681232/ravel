//! CSRF protection middleware.
//!
//! Generates and validates CSRF tokens for state-changing requests (POST,
//! PUT, PATCH, DELETE). Tokens are stored **per-session** in the shared
//! [`SessionState`] and verified via constant-time comparison.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_http::csrf::Csrf;
//! use ravel_http::session::SessionConfig;
//!
//! // CSRF middleware MUST be placed AFTER SessionService in the stack
//! // so it can access the shared SessionState.
//! let csrf = Csrf::new();
//! Route::new()
//!     .layer(session_config.layer())
//!     .middleware(csrf.middleware())
//!     .build();
//! ```

use axum::Json;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::pin::Pin;

/// Session key where the CSRF token is stored.
const CSRF_SESSION_KEY: &str = "_csrf_token";

/// CSRF token length in bytes (before base64 encoding).
const TOKEN_LEN: usize = 32;

/// Per-session CSRF protector.
///
/// Tokens are stored in the request's [`SessionState`] and verified
/// using constant-time comparison. A fresh token is generated after
/// every successful verification to mitigate replay attacks.
#[derive(Clone, Default)]
pub struct Csrf;

impl Csrf {
    /// Create a new CSRF protector.
    pub fn new() -> Self {
        Self
    }

    /// Generate a fresh CSRF token (for use in forms / headers).
    fn generate_token() -> String {
        use rand::Rng;
        let mut bytes = [0u8; TOKEN_LEN];
        rand::thread_rng().fill(&mut bytes);
        base64_encode(&bytes)
    }

    /// Read the CSRF token from session state, generating one if absent.
    fn get_or_create_token(state: &crate::session::SessionState) -> String {
        let mut guard = state.data.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(v) = guard.values.get(CSRF_SESSION_KEY)
            && let Some(token) = v.as_str()
        {
            return token.to_string();
        }
        let token = Self::generate_token();
        guard
            .values
            .insert(CSRF_SESSION_KEY.into(), serde_json::json!(token));
        state.mark_dirty();
        token
    }

    /// Replace the CSRF token in session with a fresh one.
    fn rotate_token(state: &crate::session::SessionState) -> String {
        let token = Self::generate_token();
        let mut guard = state.data.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .values
            .insert(CSRF_SESSION_KEY.into(), serde_json::json!(&token));
        state.mark_dirty();
        token
    }

    /// Verify a submitted token against the session-stored token.
    /// Uses constant-time comparison. Returns the (possibly fresh) token
    /// on success, or `None` on mismatch.
    fn verify(state: &crate::session::SessionState, submitted: &str) -> bool {
        let guard = state.data.lock().unwrap_or_else(|e| e.into_inner());
        match guard.values.get(CSRF_SESSION_KEY).and_then(|v| v.as_str()) {
            Some(stored) => constant_time_eq(stored.as_bytes(), submitted.as_bytes()),
            None => false,
        }
    }

    /// Create an Axum middleware function from this CSRF config.
    pub fn middleware(
        self,
    ) -> impl Fn(Request, Next) -> Pin<Box<dyn std::future::Future<Output = Response> + Send>>
    + Clone
    + Send
    + Sync
    + 'static {
        move |req: Request, next: Next| {
            let csrf = self.clone();
            Box::pin(async move { csrf.handle(req, next).await })
        }
    }

    async fn handle(&self, req: Request, next: Next) -> Response {
        // Only check state-changing methods
        let method = req.method().clone();
        if !is_state_changing(&method) {
            // For GET requests, ensure a token exists (lazy init)
            if let Some(state) = req
                .extensions()
                .get::<std::sync::Arc<crate::session::SessionState>>()
            {
                Self::get_or_create_token(state);
            }
            return next.run(req).await;
        }

        // Extract token from X-CSRF-TOKEN header or X-XSRF-TOKEN header
        let token = req
            .headers()
            .get("x-csrf-token")
            .or_else(|| req.headers().get("x-xsrf-token"))
            .and_then(|v| v.to_str().ok());

        // Get session state (must be injected by SessionService middleware)
        let state = req
            .extensions()
            .get::<std::sync::Arc<crate::session::SessionState>>()
            .cloned();

        match (token, state) {
            (Some(t), Some(st)) if Self::verify(&st, t) => {
                // Rotate token after successful verification
                Self::rotate_token(&st);
                next.run(req).await
            }
            _ => {
                let body = serde_json::json!({
                    "message": "CSRF token mismatch",
                    "status": 419,
                });
                (
                    StatusCode::from_u16(419).unwrap_or(StatusCode::FORBIDDEN),
                    Json(body),
                )
                    .into_response()
            }
        }
    }
}

fn is_state_changing(method: &axum::http::Method) -> bool {
    matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE")
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{SessionData, SessionState};
    use std::sync::Arc;

    fn test_state() -> Arc<SessionState> {
        Arc::new(SessionState::new(SessionData::default()))
    }

    fn test_state_with_token() -> Arc<SessionState> {
        let state = test_state();
        {
            let mut guard = state.data.lock().unwrap();
            guard.values.insert(
                CSRF_SESSION_KEY.into(),
                serde_json::json!("test-csrf-token"),
            );
        }
        state
    }

    #[test]
    fn test_generate_and_store_token() {
        let state = test_state();
        let token = Csrf::get_or_create_token(&state);

        // Token should be stored in session
        let guard = state.data.lock().unwrap();
        assert_eq!(
            guard
                .values
                .get(CSRF_SESSION_KEY)
                .unwrap()
                .as_str()
                .unwrap(),
            token
        );
        assert!(state.take_dirty());
    }

    #[test]
    fn test_token_persists_across_calls() {
        let state = test_state();
        let token1 = Csrf::get_or_create_token(&state);
        let token2 = Csrf::get_or_create_token(&state);
        // Second call returns the same token
        assert_eq!(token1, token2);
    }

    #[test]
    fn test_verify_valid_token() {
        let state = test_state_with_token();
        assert!(Csrf::verify(&state, "test-csrf-token"));
    }

    #[test]
    fn test_verify_invalid_token() {
        let state = test_state_with_token();
        assert!(!Csrf::verify(&state, "wrong-token"));
    }

    #[test]
    fn test_verify_no_token_in_session() {
        let state = test_state(); // empty session
        assert!(!Csrf::verify(&state, "anything"));
    }

    #[test]
    fn test_rotate_token_changes_value() {
        let state = test_state_with_token();
        let new_token = Csrf::rotate_token(&state);

        assert_ne!(new_token, "test-csrf-token");
        let guard = state.data.lock().unwrap();
        assert_eq!(
            guard
                .values
                .get(CSRF_SESSION_KEY)
                .unwrap()
                .as_str()
                .unwrap(),
            new_token
        );
        assert!(state.take_dirty());
    }

    #[test]
    fn test_is_state_changing() {
        use axum::http::Method;
        assert!(is_state_changing(&Method::POST));
        assert!(is_state_changing(&Method::PUT));
        assert!(is_state_changing(&Method::PATCH));
        assert!(is_state_changing(&Method::DELETE));
        assert!(!is_state_changing(&Method::GET));
        assert!(!is_state_changing(&Method::HEAD));
        assert!(!is_state_changing(&Method::OPTIONS));
    }
}
