# Cache

Ravel provides a unified caching API backed by an in-memory store. The `Cache` facade lets
you store, retrieve, and invalidate data with an optional time-to-live (TTL).

> The cache requires registration via `Application::with_cache()` before boot.

## Enabling Caching

Call `.with_cache()` on the `Application` builder. This registers a `MemoryCache` singleton
in the container.

```rust
use ravel_core::app::Application;
use ravel_facades::Cache;

Application::new()
    .with_cache()
    .register_provider(MyProvider)
    .boot()?;

// Cache is now available globally
Cache::put("key", "value", None);
```

## Basic Usage

### Storing and Retrieving

`Cache::put` accepts any value that implements `Any + Send + Sync`. `Cache::get` returns an
`Option<T>` and attempts to downcast to the requested type.

```rust
use std::time::Duration;
use ravel_facades::Cache;

// Store with no expiration
Cache::put("app:name", "Ravel", None);

// Store with a 60-second TTL
Cache::put("user:1", user_profile, Some(Duration::from_secs(60)));

// Retrieve typed value
let name: Option<&str> = Cache::get("app:name");
let profile: Option<UserProfile> = Cache::get("user:1");
```

### Checking and Removing Entries

```rust
if Cache::has("user:1") {
    println!("User is cached");
}

Cache::forget("user:1");  // Remove a specific key
Cache::flush();            // Remove all cached entries
```

## TTL — Time-to-Live

When a TTL is provided, the cache entry is automatically evicted after the duration
elapses. Passing `None` means the entry lives indefinitely (until `forget` or `flush`).

```rust
Cache::put("temp_token", token, Some(Duration::from_secs(300))); // 5 minutes
Cache::put("session", data, Some(Duration::from_secs(3600)));    // 1 hour
Cache::put("config", config, None);                              // persistent
```

## Example: Caching User Profiles

```rust
use ravel_facades::{Cache, Route};
use std::time::Duration;

async fn get_user(id: u32) -> impl IntoResponse {
    // Try cache first
    if let Some(profile) = Cache::get::<UserProfile>(&format!("user:{id}")) {
        return response().json(profile).unwrap();
    }

    // Fetch from database and cache for 10 minutes
    let profile = UserProfile::find(id).await;
    Cache::put(format!("user:{id}"), profile.clone(), Some(Duration::from_secs(600)));

    response().json(profile).unwrap()
}
```

## Example: Cache Invalidation

```rust
use ravel_facades::{Cache, Route};
use std::time::Duration;

async fn update_user(id: u32, payload: UpdateUser) -> Result<impl IntoResponse, RavelError> {
    // Update the database
    User::update(id, payload).await?;

    // Invalidate the cached profile so next read is fresh
    Cache::forget(format!("user:{id}"));

    // Also invalidate the list view
    Cache::forget("users:active");

    Ok(redirect("/users"))
}
```

## API Reference

| Method | Description |
|--------|-------------|
| `Cache::put(key, value, ttl)` | Store a value with optional `Duration` TTL |
| `Cache::get::<T>(key)` | Retrieve a typed value, returns `Option<T>` |
| `Cache::has(key)` | Check if a key exists |
| `Cache::forget(key)` | Remove a specific key |
| `Cache::flush()` | Remove all keys |

## Under the Hood

The default driver is `MemoryCache`, which stores entries in a `Mutex<HashMap>`.
Expired entries are lazily evicted on every `get` and `has` call.
