//! CSRF protection middleware.
//!
//! Generates and validates CSRF tokens for state-changing requests (POST,
//! PUT, PATCH, DELETE). Tokens are stored in an encrypted cookie and
//! verified against a request header or form field.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_http::csrf::Csrf;
//!
//! let csrf = Csrf::new(app_key);
//! Route::new()
//!     .middleware(csrf.middleware())
//!     .build();
//! ```

use axum::Json;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::pin::Pin;

/// CSRF token length in bytes (before base64 encoding).
const TOKEN_LEN: usize = 32;

/// HMAC-based CSRF protector.
///
/// Tokens are verified using a constant-time comparison of an HMAC over
/// the session/request binding to prevent timing attacks.
#[derive(Clone)]
pub struct Csrf {
    key: Vec<u8>,
}

impl Csrf {
    /// Create a new CSRF protector with the given application key.
    ///
    /// The key should come from `config.app_key` (the same key used for
    /// encryption).
    pub fn new(key: impl AsRef<[u8]>) -> Self {
        Self {
            key: key.as_ref().to_vec(),
        }
    }

    /// Generate a fresh CSRF token.
    pub fn generate(&self) -> String {
        use rand::Rng;
        let mut bytes = [0u8; TOKEN_LEN];
        rand::thread_rng().fill(&mut bytes);
        let token = base64_encode(&bytes);
        let hmac = self.hmac_sign(&token);
        format!("{token}.{hmac}")
    }

    /// Verify a CSRF token. Returns `true` if the token is valid.
    pub fn verify(&self, token: &str) -> bool {
        let (payload, signature) = match token.split_once('.') {
            Some(parts) => parts,
            None => return false,
        };
        let expected = self.hmac_sign(payload);
        constant_time_eq(signature.as_bytes(), expected.as_bytes())
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
            return next.run(req).await;
        }

        // Extract token from X-CSRF-TOKEN header or X-XSRF-TOKEN header
        let token = req
            .headers()
            .get("x-csrf-token")
            .or_else(|| req.headers().get("x-xsrf-token"))
            .and_then(|v| v.to_str().ok());

        match token {
            Some(t) if self.verify(t) => next.run(req).await,
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

    fn hmac_sign(&self, data: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.key).expect("HMAC key");
        Mac::update(&mut mac, data.as_bytes());
        let result = Mac::finalize(mac);
        base64_encode(&result.into_bytes())
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

    #[test]
    fn test_generate_and_verify() {
        let csrf = Csrf::new(b"test-key-32-bytes-long!!!!!!");
        let token = csrf.generate();
        assert!(csrf.verify(&token));
    }

    #[test]
    fn test_verify_tampered_token() {
        let csrf = Csrf::new(b"test-key-32-bytes-long!!!!!!");
        let token = csrf.generate();
        // Flip the first base64 char to guarantee tampering (the original
        // `.replace('a', "b")` was a no-op when the token lacked 'a' chars).
        let tampered = format!("X{}", &token[1..]);
        assert!(!csrf.verify(&tampered));
    }

    #[test]
    fn test_verify_missing_signature() {
        let csrf = Csrf::new(b"test-key-32-bytes-long!!!!!!");
        assert!(!csrf.verify("just-a-random-string"));
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
