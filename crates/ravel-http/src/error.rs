//! Unified error type for the Ravel HTTP layer.
//!
//! [`RavelError`] implements [`axum::response::IntoResponse`] so handlers can
//! return `Result<T, RavelError>` and use `?` propagation:
//!
//! ```rust,ignore
//! use ravel_http::error::RavelError;
//!
//! async fn show_user(id: u32) -> Result<impl IntoResponse, RavelError> {
//!     let user = find_user(id).ok_or(RavelError::not_found("User not found"))?;
//!     Ok(Json(user))
//! }
//! ```

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::collections::HashMap;
use tracing::error;

/// Unified error type that maps to HTTP responses.
#[derive(Debug)]
pub enum RavelError {
    /// 404 — resource not found.
    NotFound(String),
    /// 400 — malformed request.
    BadRequest(String),
    /// 401 — missing or invalid authentication.
    Unauthorized(String),
    /// 403 — authenticated but not permitted.
    Forbidden(String),
    /// 422 — validation failed.  Keys are field names, values are error lists.
    ValidationError(HashMap<String, Vec<String>>),
    /// 500 — unexpected internal error.
    Internal(anyhow::Error),
}

impl RavelError {
    /// Create a 404 Not Found error.
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    /// Create a 400 Bad Request error.
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }

    /// Create a 401 Unauthorized error.
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::Unauthorized(msg.into())
    }

    /// Create a 403 Forbidden error.
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Forbidden(msg.into())
    }

    /// Create a 422 Validation Error.
    pub fn validation_error(errors: HashMap<String, Vec<String>>) -> Self {
        Self::ValidationError(errors)
    }
}

impl IntoResponse for RavelError {
    fn into_response(self) -> Response {
        match self {
            Self::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"message": msg, "status": 404})),
            )
                .into_response(),
            Self::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"message": msg, "status": 400})),
            )
                .into_response(),
            Self::Unauthorized(msg) => (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"message": msg, "status": 401})),
            )
                .into_response(),
            Self::Forbidden(msg) => (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({"message": msg, "status": 403})),
            )
                .into_response(),
            Self::ValidationError(errors) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({
                    "message": "Validation failed",
                    "errors": errors,
                    "status": 422,
                })),
            )
                .into_response(),
            Self::Internal(e) => {
                error!("Internal error: {e:?}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "message": "Internal server error",
                        "status": 500,
                    })),
                )
                    .into_response()
            }
        }
    }
}

impl From<anyhow::Error> for RavelError {
    fn from(e: anyhow::Error) -> Self {
        Self::Internal(e)
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn extract_body(resp: Response) -> serde_json::Value {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let body_bytes = rt.block_on(axum::body::to_bytes(resp.into_body(), 1024)).unwrap();
        serde_json::from_slice(&body_bytes).unwrap()
    }

    #[test]
    fn test_not_found_response() {
        let resp = RavelError::not_found("User 42 not found").into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body = extract_body(resp);
        assert_eq!(body["message"], "User 42 not found");
        assert_eq!(body["status"], 404);
    }

    #[test]
    fn test_bad_request_response() {
        let resp = RavelError::bad_request("Missing field").into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = extract_body(resp);
        assert_eq!(body["status"], 400);
    }

    #[test]
    fn test_unauthorized_response() {
        let resp = RavelError::unauthorized("Invalid token").into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body = extract_body(resp);
        assert_eq!(body["status"], 401);
    }

    #[test]
    fn test_forbidden_response() {
        let resp = RavelError::forbidden("Access denied").into_response();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let body = extract_body(resp);
        assert_eq!(body["status"], 403);
    }

    #[test]
    fn test_validation_error_response() {
        let mut errors = HashMap::new();
        errors.insert("email".into(), vec!["The email field is required.".into()]);
        let resp = RavelError::validation_error(errors).into_response();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = extract_body(resp);
        assert_eq!(body["status"], 422);
        assert!(body["errors"]["email"][0].as_str().unwrap().contains("required"));
    }

    #[test]
    fn test_internal_response() {
        let err = RavelError::Internal(anyhow::anyhow!("db connection lost"));
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = extract_body(resp);
        assert_eq!(body["status"], 500);
        assert_eq!(body["message"], "Internal server error");
    }

    #[test]
    fn test_from_anyhow() {
        let err: RavelError = anyhow::anyhow!("something broke").into();
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
