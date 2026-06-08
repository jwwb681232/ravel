pub fn env(_key: &str) -> Option<String> {
    unimplemented!()
}

pub fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

pub fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}
