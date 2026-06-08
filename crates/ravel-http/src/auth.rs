//! Authentication facade — session-based and token-based auth.
//!
//! Provides [`Auth`] for checking login state and [`AuthGuard`] for
//! protecting routes.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_http::auth::Auth;
//!
//! // In your login handler:
//! async fn login(mut session: Session) -> Result<impl IntoResponse, RavelError> {
//!     let user_id = authenticate(credentials)?;
//!     Auth::login(&mut session, user_id);
//!     Ok(response().redirect("/dashboard"))
//! }
//!
//! // Check auth state:
//! async fn dashboard(session: Session) -> Result<impl IntoResponse, RavelError> {
//!     if !Auth::check(&session) {
//!         return Err(RavelError::unauthorized("Please log in"));
//!     }
//!     let user_id: u32 = Auth::id(&session).unwrap();
//!     // ...
//! }
//! ```

use crate::session::Session;
use axum::response::IntoResponse;

/// Authentication helper — stateless methods for session-based auth.
pub struct Auth;

impl Auth {
    /// Log the user in by storing their ID in the session.
    pub fn login<T: serde::Serialize>(session: &mut Session, id: T) {
        session.put("_auth_id", id);
    }

    /// Log the user out by removing auth data from the session.
    pub fn logout(session: &mut Session) {
        session.forget("_auth_id");
    }

    /// Check if the user is authenticated.
    pub fn check(session: &Session) -> bool {
        session.has("_auth_id")
    }

    /// Get the authenticated user's ID.
    pub fn id<T: serde::de::DeserializeOwned>(session: &Session) -> Option<T> {
        session.get("_auth_id")
    }

    /// Check if the session is guest (not logged in).
    pub fn guest(session: &Session) -> bool {
        !Self::check(session)
    }
}

// ── AuthGuard ───────────────────────────────────────────────────────────

/// Middleware that ensures a user is authenticated.
///
/// Returns 401 if no auth session exists.
///
/// ```rust,ignore
/// Route::new()
///     .middleware(AuthGuard::middleware())
///     .get("/dashboard", dashboard)
///     .build();
/// ```
pub struct AuthGuard;

impl AuthGuard {
    pub fn middleware() -> impl Fn(
        axum::extract::Request,
        axum::middleware::Next,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = axum::response::Response> + Send>,
    > + Clone
    + Send
    + Sync
    + 'static {
        |req: axum::extract::Request, next: axum::middleware::Next| {
            Box::pin(async move {
                let jar = crate::cookie::CookieJar::parse(
                    req.headers()
                        .get("cookie")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or(""),
                );
                if jar.get("ravel_session").is_none() {
                    return (
                        axum::http::StatusCode::UNAUTHORIZED,
                        axum::Json(serde_json::json!({
                            "message": "Unauthenticated",
                            "status": 401,
                        })),
                    )
                        .into_response();
                }
                next.run(req).await
            })
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionData;

    fn new_session() -> Session {
        Session {
            data: SessionData::default(),
            dirty: false,
        }
    }

    #[test]
    fn test_auth_login_check_logout() {
        let mut session = new_session();

        assert!(Auth::guest(&session));
        assert!(!Auth::check(&session));

        Auth::login(&mut session, 42u32);

        assert!(Auth::check(&session));
        assert!(!Auth::guest(&session));
        assert_eq!(Auth::id::<u32>(&session), Some(42));

        Auth::logout(&mut session);

        assert!(Auth::guest(&session));
        assert_eq!(Auth::id::<u32>(&session), None);
    }
}
