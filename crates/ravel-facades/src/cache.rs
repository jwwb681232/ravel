//! Cache facade — in-memory TTL cache.
use ravel_core::app::app;
use ravel_core::cache::Cache as CacheTrait;
use std::time::Duration;

pub struct Cache;

impl Cache {
    fn try_cache() -> Option<std::sync::Arc<ravel_core::cache::MemoryCache>> {
        match app() {
            Some(app) => app.container().resolve::<ravel_core::cache::MemoryCache>(),
            None => {
                tracing::warn!("Cache facade called before Application::boot()");
                None
            }
        }
    }

    fn cache() -> std::sync::Arc<ravel_core::cache::MemoryCache> {
        let app = app().expect(
            "Application not booted — call Application::boot() first"
        );
        app.container()
            .resolve::<ravel_core::cache::MemoryCache>()
            .expect(
                "MemoryCache not registered — call Application::with_cache() before boot"
            )
    }

    pub fn put(key: &str, value: impl std::any::Any + Send + Sync, ttl: Option<Duration>) {
        Self::cache().put(key, Box::new(value), ttl);
    }

    pub fn get<T: 'static + Clone + Send + Sync>(key: &str) -> Option<T> {
        Self::try_cache().and_then(|c| c.get::<T>(key))
    }

    pub fn has(key: &str) -> bool {
        Self::try_cache().map(|c| c.has(key)).unwrap_or(false)
    }

    pub fn forget(key: &str) {
        Self::cache().forget(key);
    }

    pub fn flush() {
        Self::cache().flush();
    }
}
