//! Rate limiting middleware.
//!
//! Limits the number of requests a client can make in a time window.
//! Defaults to in-memory storage; Redis/DB backends are planned.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_http::rate_limit::RateLimiter;
//! use std::time::Duration;
//!
//! let limiter = RateLimiter::per_minute(60);
//! Route::new()
//!     .middleware(limiter.middleware())
//!     .build();
//! ```

use axum::Json;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Mutex;
use std::time::{Duration, Instant};

// ── RateLimiter ──────────────────────────────────────────────────────────

/// In-memory rate limiter.
///
/// Not suitable for multi-process deployments; use Redis or DB drivers
/// for distributed setups.
pub struct RateLimiter {
    inner: std::sync::Arc<Mutex<HashMap<String, ClientState>>>,
    max_requests: u64,
    window: Duration,
}

impl Clone for RateLimiter {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            max_requests: self.max_requests,
            window: self.window,
        }
    }
}

#[derive(Clone)]
struct ClientState {
    count: u64,
    reset_at: Instant,
}

impl RateLimiter {
    /// Allow `max_requests` per `window` duration.
    pub fn new(max_requests: u64, window: Duration) -> Self {
        Self {
            inner: std::sync::Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window,
        }
    }

    /// Allow `max_requests` requests per minute.
    pub fn per_minute(max_requests: u64) -> Self {
        Self::new(max_requests, Duration::from_secs(60))
    }

    /// Allow `max_requests` requests per hour.
    pub fn per_hour(max_requests: u64) -> Self {
        Self::new(max_requests, Duration::from_secs(3600))
    }

    /// Create an Axum middleware function.
    pub fn middleware(
        self,
    ) -> impl Fn(Request, Next) -> Pin<Box<dyn std::future::Future<Output = Response> + Send>>
    + Clone
    + Send
    + Sync
    + 'static {
        move |req: Request, next: Next| {
            let limiter = self.clone();
            Box::pin(async move { limiter.handle(req, next).await })
        }
    }

    async fn handle(&self, req: Request, next: Next) -> Response {
        let client_ip = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("127.0.0.1")
            .to_string();

        let (allowed, remaining, reset) = self.check(&client_ip);

        if !allowed {
            let mut resp = (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({
                    "message": "Too Many Requests",
                    "status": 429,
                })),
            )
                .into_response();
            set_headers(&mut resp, self.max_requests, remaining, reset);
            return resp;
        }

        let mut resp = next.run(req).await;
        set_headers(&mut resp, self.max_requests, remaining, reset);
        resp
    }

    /// Check whether a request from `key` should be allowed.
    /// Returns `(allowed, remaining, reset_at)`.
    pub fn check(&self, key: &str) -> (bool, u64, Instant) {
        let mut map = self.inner.lock().unwrap();
        let now = Instant::now();
        let state = map.entry(key.to_string()).or_insert(ClientState {
            count: 0,
            reset_at: now + self.window,
        });

        if now >= state.reset_at {
            state.count = 0;
            state.reset_at = now + self.window;
        }

        state.count += 1;
        let remaining = self.max_requests.saturating_sub(state.count);
        (state.count <= self.max_requests, remaining, state.reset_at)
    }
}

fn set_headers(resp: &mut Response, max: u64, remaining: u64, reset: Instant) {
    let headers = resp.headers_mut();
    if let Ok(v) = max.to_string().parse() {
        headers.insert("X-RateLimit-Limit", v);
    }
    if let Ok(v) = remaining.to_string().parse() {
        headers.insert("X-RateLimit-Remaining", v);
    }
    if let Ok(v) = reset
        .duration_since(Instant::now())
        .as_secs()
        .to_string()
        .parse()
    {
        headers.insert("X-RateLimit-Reset", v);
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allows_within_limit() {
        let limiter = RateLimiter::new(5, Duration::from_secs(60));
        for _ in 0..5 {
            let (allowed, remaining, _) = limiter.check("127.0.0.1");
            assert!(allowed);
            assert!(remaining < 5);
        }
    }

    #[test]
    fn test_blocks_after_limit() {
        let limiter = RateLimiter::new(3, Duration::from_secs(60));
        for _ in 0..3 {
            assert!(limiter.check("10.0.0.1").0);
        }
        let (allowed, remaining, _) = limiter.check("10.0.0.1");
        assert!(!allowed);
        assert_eq!(remaining, 0);
    }

    #[test]
    fn test_isolated_per_client() {
        let limiter = RateLimiter::new(2, Duration::from_secs(60));
        assert!(limiter.check("a").0);
        assert!(limiter.check("b").0);
        assert!(limiter.check("a").0);
        assert!(!limiter.check("a").0);
        assert!(limiter.check("b").0);
        assert!(!limiter.check("b").0);
    }

    #[test]
    fn test_window_reset() {
        // Use a very short window so it expires during the test
        let limiter = RateLimiter::new(2, Duration::from_millis(10));
        assert!(limiter.check("c").0);
        assert!(limiter.check("c").0);
        assert!(!limiter.check("c").0);
        std::thread::sleep(Duration::from_millis(20));
        assert!(limiter.check("c").0);
    }
}
