//! Cache abstraction — trait-based caching with an in-memory implementation.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_core::cache::{Cache, MemoryCache};
//! use std::time::Duration;
//!
//! let cache = MemoryCache::new();
//! cache.put("key", "value", Some(Duration::from_secs(60)));
//! assert_eq!(cache.get::<String>("key"), Some("value".into()));
//! ```

use std::any::Any;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Universal cache trait — implement for Redis, Memcached, in-memory, etc.
pub trait Cache: Send + Sync {
    /// Store a value with an optional TTL.
    fn put(&self, key: &str, value: Box<dyn Any + Send + Sync>, ttl: Option<Duration>);

    /// Retrieve a typed value by key.
    fn get<T>(&self, key: &str) -> Option<T>
    where
        T: 'static + Clone + Send + Sync;

    /// Check whether a key exists.
    fn has(&self, key: &str) -> bool;

    /// Remove a key.
    fn forget(&self, key: &str);

    /// Remove all keys.
    fn flush(&self);
}

// ── MemoryCache ────────────────────────────────────────────────────

struct CacheEntry {
    value: Box<dyn Any + Send + Sync>,
    expires_at: Option<Instant>,
}

/// Simple in-memory cache suitable for single-instance deployments.
pub struct MemoryCache {
    store: Mutex<HashMap<String, CacheEntry>>,
}

impl Default for MemoryCache {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryCache {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }

    fn evict_expired(store: &mut HashMap<String, CacheEntry>) {
        let now = Instant::now();
        store.retain(|_, entry| entry.expires_at.is_none_or(|t| t > now));
    }
}

impl Cache for MemoryCache {
    fn put(&self, key: &str, value: Box<dyn Any + Send + Sync>, ttl: Option<Duration>) {
        let mut store = self.store.lock().unwrap();
        let expires_at = ttl.map(|d| Instant::now() + d);
        store.insert(key.to_string(), CacheEntry { value, expires_at });
    }

    fn get<T>(&self, key: &str) -> Option<T>
    where
        T: 'static + Clone + Send + Sync,
    {
        let mut store = self.store.lock().unwrap();
        Self::evict_expired(&mut store);

        store
            .get(key)
            .and_then(|entry| entry.value.downcast_ref::<T>().cloned())
    }

    fn has(&self, key: &str) -> bool {
        let mut store = self.store.lock().unwrap();
        Self::evict_expired(&mut store);
        store.contains_key(key)
    }

    fn forget(&self, key: &str) {
        self.store.lock().unwrap().remove(key);
    }

    fn flush(&self) {
        self.store.lock().unwrap().clear();
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_put_and_get() {
        let cache = MemoryCache::new();
        cache.put("hello", Box::new("world".to_string()), None);

        let val: Option<String> = cache.get("hello");
        assert_eq!(val, Some("world".into()));
    }

    #[test]
    fn test_has() {
        let cache = MemoryCache::new();
        assert!(!cache.has("x"));
        cache.put("x", Box::new(42u32), None);
        assert!(cache.has("x"));
    }

    #[test]
    fn test_forget() {
        let cache = MemoryCache::new();
        cache.put("temp", Box::new("data"), None);
        cache.forget("temp");
        assert!(!cache.has("temp"));
    }

    #[test]
    fn test_flush() {
        let cache = MemoryCache::new();
        cache.put("a", Box::new(1), None);
        cache.put("b", Box::new(2), None);
        cache.flush();
        assert!(!cache.has("a"));
        assert!(!cache.has("b"));
    }

    #[test]
    fn test_ttl_expires() {
        let cache = MemoryCache::new();
        cache.put("short", Box::new("live"), Some(Duration::from_millis(1)));
        std::thread::sleep(Duration::from_millis(10));

        let val: Option<String> = cache.get("short");
        assert_eq!(val, None);
    }

    #[test]
    fn test_typed_integers() {
        let cache = MemoryCache::new();
        cache.put("count", Box::new(100u64), None);
        assert_eq!(cache.get::<u64>("count"), Some(100));
    }
}
