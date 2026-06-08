//! Config facade — dot-notation configuration access.
use ravel_core::app::APP;
use serde::de::DeserializeOwned;

pub struct Config;

impl Config {
    pub fn get<T: DeserializeOwned>(key: &str) -> Option<T> {
        let app = APP.get().expect("Application not booted — call Application::boot() first");
        app.config().get(key)
    }

    pub fn get_or<T: DeserializeOwned>(key: &str, default: T) -> T {
        Self::get(key).unwrap_or(default)
    }

    pub fn has(key: &str) -> bool {
        let app = APP.get().expect("Application not booted — call Application::boot() first");
        app.config().has(key)
    }
}
