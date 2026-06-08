//! Cookie management — read and set cookies from handlers.
//!
//! [`CookieJar`] is an Axum extractor for reading cookies. Use
//! [`SetCookie`] to add cookies to responses.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_http::cookie::{CookieJar, SetCookie};
//!
//! async fn handler(jar: CookieJar) -> (SetCookie, &str) {
//!     let theme = jar.get("theme").unwrap_or("light");
//!     (SetCookie::new("last_visit", "now").secure(true), "ok")
//! }
//! ```

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use std::collections::HashMap;

// ── CookieJar ───────────────────────────────────────────────────────────

/// Extractor that parses cookies from incoming requests.
///
/// Implements [`FromRequestParts`] so it can be used directly as a handler
/// parameter alongside other extractors.
#[derive(Debug, Clone)]
pub struct CookieJar {
    cookies: HashMap<String, String>,
}

impl CookieJar {
    /// Get a cookie value by name.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.cookies.get(name).map(|s| s.as_str())
    }

    /// Check if a cookie exists.
    pub fn has(&self, name: &str) -> bool {
        self.cookies.contains_key(name)
    }

    /// Iterate over all cookies.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.cookies.iter()
    }

    pub(crate) fn parse(cookie_header: &str) -> Self {
        let mut cookies = HashMap::new();
        for pair in cookie_header.split(';') {
            let pair = pair.trim();
            if let Some((k, v)) = pair.split_once('=') {
                cookies.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
        Self { cookies }
    }
}

impl<S: Send + Sync> FromRequestParts<S> for CookieJar {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let jar = parts
            .headers
            .get("cookie")
            .and_then(|v| v.to_str().ok())
            .map(CookieJar::parse)
            .unwrap_or(CookieJar {
                cookies: HashMap::new(),
            });
        Ok(jar)
    }
}

// ── SetCookie ───────────────────────────────────────────────────────────

/// A single cookie to be set on the response.
#[derive(Debug, Clone)]
pub struct SetCookie {
    name: String,
    value: String,
    http_only: bool,
    secure: bool,
    same_site: SameSite,
    max_age_secs: Option<u64>,
    path: String,
}

#[derive(Debug, Clone)]
enum SameSite {
    Strict,
    Lax,
    None,
}

impl SetCookie {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            http_only: true,
            secure: false,
            same_site: SameSite::Lax,
            max_age_secs: None,
            path: "/".into(),
        }
    }

    pub fn http_only(mut self, v: bool) -> Self {
        self.http_only = v;
        self
    }

    pub fn secure(mut self, v: bool) -> Self {
        self.secure = v;
        self
    }

    pub fn same_site_strict(mut self) -> Self {
        self.same_site = SameSite::Strict;
        self
    }

    pub fn same_site_none(mut self) -> Self {
        self.same_site = SameSite::None;
        self
    }

    pub fn max_age(mut self, secs: u64) -> Self {
        self.max_age_secs = Some(secs);
        self
    }

    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    fn to_header_value(&self) -> String {
        let mut s = format!("{}={}", self.name, self.value);
        if self.http_only {
            s.push_str("; HttpOnly");
        }
        if self.secure {
            s.push_str("; Secure");
        }
        match self.same_site {
            SameSite::Strict => s.push_str("; SameSite=Strict"),
            SameSite::Lax => s.push_str("; SameSite=Lax"),
            SameSite::None => s.push_str("; SameSite=None"),
        }
        if let Some(age) = self.max_age_secs {
            s.push_str(&format!("; Max-Age={age}"));
        }
        s.push_str(&format!("; Path={}", self.path));
        s
    }
}

/// A collection of [`SetCookie`] values for the response.
#[derive(Debug)]
pub struct CookieSetter {
    cookies: Vec<SetCookie>,
}

impl CookieSetter {
    pub fn new() -> Self {
        Self {
            cookies: Vec::new(),
        }
    }

    pub fn add(&mut self, cookie: SetCookie) {
        self.cookies.push(cookie);
    }
}

impl Default for CookieSetter {
    fn default() -> Self {
        Self::new()
    }
}

impl CookieSetter {
    /// Attach cookies to a response and return it.
    pub fn attach<T: IntoResponse>(&self, response: T) -> Response {
        let mut resp = response.into_response();
        let headers = resp.headers_mut();
        for cookie in &self.cookies {
            if let Ok(v) = cookie.to_header_value().parse() {
                headers.append("Set-Cookie", v);
            }
        }
        resp
    }
}

/// Response wrapper that carries a [`SetCookie`] header.
pub struct CookieResponse<T: IntoResponse> {
    cookie: SetCookie,
    inner: T,
}

impl<T: IntoResponse> CookieResponse<T> {
    pub fn new(cookie: SetCookie, inner: T) -> Self {
        Self { cookie, inner }
    }
}

impl<T: IntoResponse> IntoResponse for CookieResponse<T> {
    fn into_response(self) -> Response {
        let mut resp = self.inner.into_response();
        if let Ok(v) = self.cookie.to_header_value().parse() {
            resp.headers_mut().append("Set-Cookie", v);
        }
        resp
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn test_cookie_jar_parse() {
        let jar = CookieJar::parse("theme=dark; lang=en");
        assert_eq!(jar.get("theme"), Some("dark"));
        assert_eq!(jar.get("lang"), Some("en"));
        assert!(jar.get("missing").is_none());
    }

    #[test]
    fn test_cookie_jar_empty() {
        let jar = CookieJar::parse("");
        assert!(jar.get("anything").is_none());
    }

    #[test]
    fn test_set_cookie_header() {
        let cookie = SetCookie::new("session", "abc123")
            .http_only(true)
            .secure(true)
            .same_site_strict()
            .max_age(3600);
        let header = cookie.to_header_value();
        assert!(header.contains("session=abc123"));
        assert!(header.contains("HttpOnly"));
        assert!(header.contains("Secure"));
        assert!(header.contains("SameSite=Strict"));
        assert!(header.contains("Max-Age=3600"));
    }

    #[tokio::test]
    async fn test_cookie_response() {
        let resp =
            CookieResponse::new(SetCookie::new("x", "y"), (StatusCode::OK, "ok")).into_response();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(
            resp.headers()
                .get("Set-Cookie")
                .unwrap()
                .to_str()
                .unwrap()
                .contains("x=y")
        );
    }

    #[test]
    fn test_cookie_setter_attach() {
        let mut setter = CookieSetter::new();
        setter.add(SetCookie::new("a", "1"));
        setter.add(SetCookie::new("b", "2"));
        let resp = setter.attach((StatusCode::OK, "ok"));
        let headers: Vec<_> = resp.headers().get_all("Set-Cookie").iter().collect();
        assert_eq!(headers.len(), 2);
    }
}
