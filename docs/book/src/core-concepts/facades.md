# Facades

Facades provide a **static** ("global") interface to services in the container. Instead of manually resolving services, you call methods directly on the facade struct:

```rust
use ravel_facades::{Config, Cache, Hash, Crypt, Log};

// Instead of: app.config().get("app.name")
let name: String = Config::get("app.name").unwrap();

// Instead of: let cache = container.resolve::<MemoryCache>(); cache.put(...)
Cache::put("key", "value", None);

// No container needed at all
let hashed = Hash::make("password").unwrap();
Log::info!("Application started");
```

## How Facades Work

Facades are **zero-size structs** whose methods read from a global `Application` singleton. After `Application::boot()`, the application is stored in a `OnceLock` static. Each facade method accesses it:

```rust
// Internally (simplified):
pub struct Config;

impl Config {
    pub fn get<T: DeserializeOwned>(key: &str) -> Option<T> {
        APP.get()                    // static OnceLock<Application>
            .expect("Application not booted")
            .config()                // &ConfigRepo
            .get(key)                // Option<T>
    }
}
```

## Available Facades

| Facade | Service | Requires |
|--------|---------|----------|
| `Config` | Configuration repository | None (always available) |
| `Hash` | bcrypt password hashing | None (zero-setup) |
| `Log` | Structured logging (tracing) | None (zero-setup) |
| `Storage` | File storage (LocalDisk) | None (zero-setup) |
| `Cache` | In-memory TTL cache | `Application::with_cache()` |
| `Crypt` | AES-256-GCM encryption | `Application::with_app_key()` |
| `Queue` | Job queue dispatch | `Application::with_queue()` (via `ApplicationExt`) |

## Request-Level Facades

Some facades depend on the **current HTTP request**. These use `tokio::task_local!` — a per-task storage mechanism set by a framework middleware:

| Facade | Depends On |
|--------|-----------|
| `Auth` | Current request's session cookie |
| `Session` | Current request's encrypted session data |
| `request()` | Current request's method, path, query, headers |
| `Route` | Global route registry (write during boot, read at build) |

These facades are only available inside HTTP handlers (within the request lifecycle). Outside request scope (CLI commands, queue jobs), they return sensible defaults:

```rust
// Inside a handler:
async fn dashboard() -> impl IntoResponse {
    if Auth::check() {
        format!("Welcome, {:?}!", Auth::id::<String>())
    } else {
        redirect("/login")
    }
}

// In a CLI command:
fn my_command() {
    assert_eq!(Auth::check(), false);   // always false outside request scope
    assert_eq!(Session::get::<String>("key"), None); // always None
}
```

## Testing Facades

Each test must reset the global application state. Use the test lock pattern:

```rust
use std::sync::Mutex;
static TEST_LOCK: Mutex<()> = Mutex::new(());
fn lock() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[test]
fn test_my_feature() {
    let _l = lock();
    ravel_core::app::APP.reset();
    Route::reset();

    Application::new()
        .with_cache()
        .register_provider(MyProvider)
        .boot()
        .unwrap();

    // Use facades...
    Cache::put("test", "value", None);
    assert_eq!(Cache::get::<String>("test"), Some("value".into()));
}
```

Tests must be serialized because the global `APP` is shared across all tests. Use `--test-threads=1` or the mutex lock pattern shown above.
