//! Config facade — dot-notation configuration access.
use ravel_core::app::app;
use serde::de::DeserializeOwned;

pub struct Config;

impl Config {
    /// Resolve the app. Returns `None` + log warning if not booted.
    fn try_app() -> Option<std::sync::Arc<ravel_core::app::Application>> {
        match app() {
            Some(a) => Some(a),
            None => {
                tracing::warn!("Config facade called before Application::boot()");
                None
            }
        }
    }

    pub fn get<T: DeserializeOwned>(key: &str) -> Option<T> {
        Self::try_app().and_then(|app| app.config().get(key))
    }

    pub fn get_or<T: DeserializeOwned>(key: &str, default: T) -> T {
        Self::get(key).unwrap_or(default)
    }

    pub fn has(key: &str) -> bool {
        Self::try_app()
            .map(|app| app.config().has(key))
            .unwrap_or(false)
    }
}
