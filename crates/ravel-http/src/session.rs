//! Session management — extractor for reading/writing session data.
//!
//! Sessions are stored in an encrypted cookie by default. The session
//! data is serialised as JSON, encrypted, and stored client-side.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_http::session::Session;
//! use ravel_core::crypt::Crypt;
//!
//! let crypt = Crypt::from_key(app_key)?;
//! let session_config = SessionConfig::new(crypt);
//!
//! // Apply to routes:
//! Route::new()
//!     .layer(session_config.layer())
//!     .build();
//!
//! // In handler:
//! async fn handler(mut session: Session) -> impl IntoResponse {
//!     session.put("user_id", 42u32);
//!     session.flash("success", "Logged in!");
//!     "ok"
//! }
//! ```

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::response::Response;
use ravel_core::crypt::Crypt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tower::Layer;

// ── Session data ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct SessionData {
    values: HashMap<String, serde_json::Value>,
    flash: HashMap<String, serde_json::Value>,
    #[serde(default)]
    flash_consumed: HashMap<String, serde_json::Value>,
}

// ── SessionConfig ───────────────────────────────────────────────────────

/// Configuration for the session middleware.
#[derive(Clone)]
pub struct SessionConfig {
    crypt: Arc<Crypt>,
    cookie_name: String,
    max_age_secs: u64,
}

impl SessionConfig {
    /// Create a new session config with the given encrypter.
    pub fn new(crypt: Crypt) -> Self {
        Self {
            crypt: Arc::new(crypt),
            cookie_name: "ravel_session".into(),
            max_age_secs: 7200, // 2 hours
        }
    }

    /// Set the session cookie name (default: "ravel_session").
    pub fn cookie_name(mut self, name: impl Into<String>) -> Self {
        self.cookie_name = name.into();
        self
    }

    /// Set the session max age in seconds (default: 7200).
    pub fn max_age(mut self, secs: u64) -> Self {
        self.max_age_secs = secs;
        self
    }

    /// Produce a Tower Layer for this config.
    pub fn layer(&self) -> SessionLayer {
        SessionLayer {
            config: self.clone(),
        }
    }

    fn read(&self, cookie_header: Option<&str>) -> SessionData {
        let raw = cookie_header.and_then(|h| {
            h.split(';')
                .map(|p| p.trim())
                .find_map(|p| p.strip_prefix(&format!("{}=", self.cookie_name)))
                .map(|v| v.to_string())
        });

        match raw {
            Some(encoded) => self
                .crypt
                .decrypt_value::<SessionData>(&encoded)
                .unwrap_or_default(),
            None => SessionData::default(),
        }
    }

    fn write(&self, data: &SessionData) -> String {
        let encoded = self.crypt.encrypt_value(data).unwrap_or_default();
        let mut cookie = format!(
            "{}={}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}",
            self.cookie_name, encoded, self.max_age_secs
        );
        // Mark secure in production (non-localhost)
        cookie.push_str("; Secure");
        cookie
    }
}

// ── SessionLayer ────────────────────────────────────────────────────────

/// Tower Layer that injects session data into requests.
#[derive(Clone)]
pub struct SessionLayer {
    config: SessionConfig,
}

impl<S: Clone + Send + Sync + 'static> Layer<S> for SessionLayer {
    type Service = SessionService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SessionService {
            inner,
            config: self.config.clone(),
        }
    }
}

// ── SessionService ──────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SessionService<S> {
    inner: S,
    config: SessionConfig,
}

impl<S, B> tower::Service<axum::http::Request<B>> for SessionService<S>
where
    S: tower::Service<axum::http::Request<B>, Response = Response> + Clone + Send + 'static,
    S::Future: Send,
    B: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: axum::http::Request<B>) -> Self::Future {
        let config = self.config.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            let cookie_header = req.headers().get("cookie").and_then(|v| v.to_str().ok());
            let data = config.read(cookie_header);
            let set_cookie = config.write(&data);

            req.extensions_mut().insert(data);

            let mut resp = inner.call(req).await?;

            // If session was modified, set the cookie
            if let Some(_modified) = resp.extensions().get::<bool>() {
                resp.headers_mut().insert(
                    "Set-Cookie",
                    set_cookie.parse().unwrap(),
                );
            }

            Ok(resp)
        })
    }
}

// ── Session extractor ───────────────────────────────────────────────────

/// Handler extractor for reading and writing session data.
///
/// Must be used after [`SessionLayer`] is applied to the route.
#[derive(Debug)]
pub struct Session {
    pub(crate) data: SessionData,
    pub(crate) dirty: bool,
}

impl Session {
    /// Get a value from the session.
    pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.data
            .values
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Store a value in the session.
    pub fn put<T: serde::Serialize>(&mut self, key: &str, value: T) {
        if let Ok(v) = serde_json::to_value(value) {
            self.data.values.insert(key.into(), v);
            self.dirty = true;
        }
    }

    /// Remove a key from the session.
    pub fn forget(&mut self, key: &str) {
        self.data.values.remove(key);
        self.dirty = true;
    }

    /// Store a flash message (available only on the next request).
    pub fn flash(&mut self, key: &str, value: impl Serialize) {
        if let Ok(v) = serde_json::to_value(value) {
            self.data.flash.insert(key.into(), v);
            self.dirty = true;
        }
    }

    /// Consume flash messages (called automatically after first read).
    pub fn flashed<T: serde::de::DeserializeOwned>(&mut self, key: &str) -> Option<T> {
        // Move current flash to consumed, return the value
        if let Some(v) = self.data.flash.remove(key) {
            self.data.flash_consumed.insert(key.into(), v.clone());
            self.dirty = true;
            return serde_json::from_value(v).ok();
        }
        None
    }

    /// Check if a key exists in the session.
    pub fn has(&self, key: &str) -> bool {
        self.data.values.contains_key(key)
    }

    /// Get all session keys.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.data.values.keys()
    }
}

impl<S: Send + Sync + 'static> FromRequestParts<S> for Session {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let data = parts
            .extensions
            .get::<SessionData>()
            .cloned()
            .unwrap_or_default();
        Ok(Session {
            data,
            dirty: false,
        })
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    fn test_crypt() -> Crypt {
        let mut key = [0u8; 32];
        rand::thread_rng().fill(&mut key);
        Crypt::new(&key)
    }

    #[test]
    fn test_session_read_write_roundtrip() {
        let config = SessionConfig::new(test_crypt());

        let mut data = SessionData::default();
        data.values
            .insert("user_id".into(), serde_json::json!(42));

        let cookie = config.write(&data);

        // write() returns the full Set-Cookie header value
        let restored = config.read(Some(&cookie));

        assert_eq!(
            restored.values.get("user_id").unwrap(),
            &serde_json::json!(42)
        );
    }

    #[test]
    fn test_session_put_and_get() {
        let mut session = Session {
            data: SessionData::default(),
            dirty: false,
        };

        session.put("name", "Alice");
        session.put("count", 42u32);

        assert_eq!(session.get::<String>("name").unwrap(), "Alice");
        assert_eq!(session.get::<u32>("count").unwrap(), 42);
        assert!(session.dirty);
    }

    #[test]
    fn test_session_flash() {
        let mut session = Session {
            data: SessionData::default(),
            dirty: false,
        };

        session.flash("success", "Operation completed");

        let msg: Option<String> = session.flashed("success");
        assert_eq!(msg.unwrap(), "Operation completed");

        // Second read should return None
        let msg: Option<String> = session.flashed("success");
        assert!(msg.is_none());
    }

    #[test]
    fn test_session_forget() {
        let mut session = Session {
            data: SessionData::default(),
            dirty: false,
        };

        session.put("key", "value");
        assert!(session.has("key"));

        session.forget("key");
        assert!(!session.has("key"));
    }
}
