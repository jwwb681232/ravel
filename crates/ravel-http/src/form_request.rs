//! FormRequest — Laravel-style request validation with automatic 422 responses.
//!
//! Implement the [`FormRequest`] trait on your struct, then use
//! [`Validated<T>`](Validated) as an Axum extractor to get automatic
//! JSON parsing + validation.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_http::form_request::{FormRequest, Validated};
//! use ravel_http::validation::{FieldRule, Rule};
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! pub struct CreateUserRequest {
//!     pub name: String,
//!     pub email: String,
//! }
//!
//! impl FormRequest for CreateUserRequest {
//!     fn rules() -> Vec<FieldRule> {
//!         vec![
//!             FieldRule::new("name", vec![Rule::Required, Rule::Min(3)]),
//!             FieldRule::new("email", vec![Rule::Required, Rule::Email]),
//!         ]
//!     }
//! }
//!
//! // In your handler:
//! async fn store(Validated(req): Validated<CreateUserRequest>) -> impl IntoResponse {
//!     // req is guaranteed to be valid
//!     format!("Hello, {}!", req.name)
//! }
//! ```
//!
//! When validation fails, a 422 Unprocessable Entity response is returned:
//!
//! ```json
//! {
//!     "message": "Validation failed",
//!     "errors": {
//!         "name": ["name is required"],
//!         "email": ["email must be a valid email address"]
//!     }
//! }
//! ```

use axum::extract::{FromRequest, Request};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

use crate::error::RavelError;
use crate::validation::{FieldRule, Validator};

/// Trait for validated request structs.
///
/// Implement this on a `#[derive(Deserialize)]` struct to define
/// validation rules.  Use [`Validated<T>`](Validated) in your handler
/// signature to trigger automatic parsing + validation:
///
/// ```rust,ignore
/// async fn store(Validated(req): Validated<CreateUserRequest>) -> impl IntoResponse {
///     format!("Hello, {}!", req.name)
/// }
/// ```
///
/// When validation fails, a [`RavelError`] is returned — the handler's
/// return type should be `Result<impl IntoResponse, RavelError>`.
pub trait FormRequest: DeserializeOwned + Send + 'static {
    /// Define validation rules for each field.
    fn rules() -> Vec<FieldRule>;

    /// Check whether the request is authorised.
    ///
    /// Return `false` to reject the request with 403 Forbidden.
    /// Default: `true` (always authorised).
    fn authorize(&self) -> bool {
        true
    }

    /// Custom error messages keyed by `"{field}.{rule}"`.
    ///
    /// Example: `"name.required" => "Please enter your name"`.
    fn messages() -> HashMap<String, String> {
        HashMap::new()
    }
}

// ── Validated<T> extractor ───────────────────────────────────────────────

/// An Axum extractor that parses JSON and validates it using [`FormRequest`].
///
/// Use as `Validated(req)` in handler parameters.  Rejection is a
/// [`RavelError`], so your handler should return
/// `Result<impl IntoResponse, RavelError>`.
///
/// ```rust,ignore
/// async fn store(
///     Validated(req): Validated<CreateUserRequest>,
/// ) -> Result<impl IntoResponse, RavelError> {
///     Ok(format!("Hello, {}!", req.name))
/// }
/// ```
#[derive(Debug)]
pub struct Validated<T>(pub T);

/// Rejection type for [`Validated`].
///
/// No longer used directly — [`Validated`] now rejects with [`RavelError`].
/// Kept for backward compatibility.
#[deprecated(since = "0.2.0", note = "`Validated<T>` now rejects with `RavelError`")]
#[derive(Debug)]
pub enum FormRequestRejection {
    BadRequest(String),
    Forbidden,
    ValidationFailed(Vec<crate::validation::ValidationError>),
}

#[allow(deprecated)]
impl IntoResponse for FormRequestRejection {
    fn into_response(self) -> Response {
        match self {
            Self::BadRequest(msg) => {
                let body = serde_json::json!({
                    "message": msg
                });
                (StatusCode::BAD_REQUEST, Json(body)).into_response()
            }
            Self::Forbidden => {
                let body = serde_json::json!({
                    "message": "Forbidden"
                });
                (StatusCode::FORBIDDEN, Json(body)).into_response()
            }
            Self::ValidationFailed(errors) => {
                let mut error_map: HashMap<String, Vec<String>> = HashMap::new();
                for e in &errors {
                    error_map
                        .entry(e.field.clone())
                        .or_default()
                        .push(e.message.clone());
                }
                let body = serde_json::json!({
                    "message": "Validation failed",
                    "errors": error_map
                });
                (StatusCode::UNPROCESSABLE_ENTITY, Json(body)).into_response()
            }
        }
    }
}

impl<S, T> FromRequest<S> for Validated<T>
where
    S: Send + Sync + 'static,
    T: FormRequest,
{
    type Rejection = RavelError;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let (_parts, body) = req.into_parts();
        let bytes = axum::body::to_bytes(body, 1024 * 1024)
            .await
            .map_err(|e| RavelError::bad_request(format!("Failed to read body: {e}")))?;

        let json_value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| RavelError::bad_request(format!("Invalid JSON: {e}")))?;

        let rules = T::rules();
        let custom_messages = T::messages();
        let validator = Validator::new(rules);

        let validation_result = if custom_messages.is_empty() {
            validator.validate(&json_value)
        } else {
            validator.validate_with_messages(&json_value, &custom_messages)
        };

        if let Err(errs) = validation_result {
            let mut error_map: HashMap<String, Vec<String>> = HashMap::new();
            for e in &errs {
                error_map.entry(e.field.clone()).or_default().push(e.message.clone());
            }
            return Err(RavelError::ValidationError(error_map));
        }

        let parsed: T = serde_json::from_value(json_value)
            .map_err(|e| RavelError::bad_request(format!("Deserialization error: {e}")))?;

        if !parsed.authorize() {
            return Err(RavelError::forbidden("Forbidden"));
        }

        Ok(Validated(parsed))
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::RavelError;
    use crate::validation::{FieldRule, Rule};
    use axum::body::Body;
    use axum::http::Request;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct TestForm {
        name: String,
        email: String,
    }

    impl FormRequest for TestForm {
        fn rules() -> Vec<FieldRule> {
            vec![
                FieldRule::new("name", vec![Rule::Required, Rule::Min(2)]),
                FieldRule::new("email", vec![Rule::Required, Rule::Email]),
            ]
        }
    }

    fn make_json_request(body: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/test")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    #[tokio::test]
    async fn test_valid_request() {
        let req = make_json_request(r#"{"name":"Alice","email":"a@b.com"}"#);
        let result = Validated::<TestForm>::from_request(req, &()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0.name, "Alice");
    }

    #[tokio::test]
    async fn test_missing_field() {
        let req = make_json_request(r#"{"email":"a@b.com"}"#);
        let result = Validated::<TestForm>::from_request(req, &()).await;
        assert!(result.is_err());

        let resp = result.unwrap_err().into_response();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn test_invalid_email() {
        let req = make_json_request(r#"{"name":"Alice","email":"invalid"}"#);
        let result = Validated::<TestForm>::from_request(req, &()).await;
        assert!(result.is_err());

        let resp = result.unwrap_err().into_response();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn test_invalid_json() {
        let req = make_json_request("not json");
        let result = Validated::<TestForm>::from_request(req, &()).await;
        assert!(result.is_err());

        let resp = result.unwrap_err().into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[derive(Debug, Deserialize)]
    struct AdminForm {
        role: String,
    }

    impl FormRequest for AdminForm {
        fn rules() -> Vec<FieldRule> {
            vec![FieldRule::new("role", vec![Rule::Required])]
        }

        fn authorize(&self) -> bool {
            self.role != "blocked"
        }
    }

    #[tokio::test]
    async fn test_authorization_pass() {
        let req = make_json_request(r#"{"role":"admin"}"#);
        let result = Validated::<AdminForm>::from_request(req, &()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_authorization_fail() {
        let req = make_json_request(r#"{"role":"blocked"}"#);
        let result = Validated::<AdminForm>::from_request(req, &()).await;
        assert!(result.is_err());

        let resp = result.unwrap_err().into_response();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct CustomMsgForm {
        name: String,
    }

    impl FormRequest for CustomMsgForm {
        fn rules() -> Vec<FieldRule> {
            vec![FieldRule::new("name", vec![Rule::Required])]
        }

        fn messages() -> HashMap<String, String> {
            let mut m = HashMap::new();
            m.insert("name.required".into(), "Please provide a name".into());
            m
        }
    }

    #[tokio::test]
    async fn test_custom_messages() {
        let req = make_json_request(r#"{}"#);
        let result = Validated::<CustomMsgForm>::from_request(req, &()).await;
        match result.unwrap_err() {
            RavelError::ValidationError(errors) => {
                assert_eq!(errors.get("name").unwrap()[0], "Please provide a name");
            }
            _ => panic!("Expected validation failure"),
        }
    }

}
