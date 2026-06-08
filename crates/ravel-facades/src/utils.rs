//! Utility helpers — env(), env_or(), now().
use chrono::Utc;

pub fn env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

pub fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

pub fn now() -> chrono::DateTime<Utc> {
    Utc::now()
}
