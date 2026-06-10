# Facades

Facades provide a **static** interface to services in the container. Instead of manually resolving services, you call methods directly on the facade struct.

```rust
use ravel_facades::{Config, Cache, Route, Crypt, Hash, Log, Session};

// Configuration
let port: u16 = Config::get_or("server.port", 3000);
let debug: bool = Config::get("app.debug").unwrap_or(false);

// Caching
Cache::put("user:1", user_data, Some(Duration::from_secs(3600)));
let cached: Option<User> = Cache::get("user:1");

// Encryption
let encrypted = Crypt::encrypt(b"secret")?;
let decrypted = Crypt::decrypt(&encrypted)?;

// Hashing
let hashed = Hash::make("password")?;
assert!(Hash::check("password", &hashed)?);

// Routing
Route::get("/", handler);
Route::group("/admin", || {
    Route::get("/dashboard", admin_dashboard);
});
let router = Route::build();
```

## Available Facades

| Facade | Crate | Description |
|--------|-------|-------------|
| `Config` | `ravel-facades` | TOML configuration access (`get`, `get_or`, `has`) |
| `Cache` | `ravel-facades` | In-memory TTL cache (`put`, `get`, `has`, `forget`, `flush`) |
| `Route` | `ravel-facades` | Static route builder (`get`, `post`, `put`, `delete`, `patch`, `group`, `middleware`, `build`) |
| `Crypt` | `ravel-facades` | AES-256-GCM encryption (`encrypt`, `decrypt`, `encrypt_value`, `decrypt_value`) |
| `Hash` | `ravel-facades` | bcrypt password hashing (`make`, `check`) |
| `Log` | `ravel-facades` | Structured logging (Tracing) |
| `Session` | `ravel-facades` | Per-request session data |
| `Auth` | `ravel-facades` | Authentication helpers |
| `Queue` | `ravel-facades` | Job dispatch |
| `Storage` | `ravel-facades` | File storage operations |

## Graceful Degradation

Read-only facades (`Config`, `Cache`) gracefully degrade when called before `Application::boot()` — they return `None` / `false` and log a warning rather than panicking:

```rust
// Before boot: no crash — returns None with a tracing warning
let debug: Option<bool> = Config::get("app.debug"); // → None
let has_key = Config::has("server.port");           // → false
let cached: Option<User> = Cache::get("user:1");    // → None
```

Write operations (`Cache::put`, `Crypt::encrypt`, `Queue::dispatch`) still panic on missing configuration — silent write failures are more dangerous.

## Route Introspection

The Route facade records all registered routes for debugging and introspection:

```rust
Route::get("/", home);
Route::post("/users", create_user);

let routes = Route::list();
for entry in &routes {
    println!("{} {}", entry.method, entry.path);
}
// → GET /
// → POST /users
```

## How Facades Work

Under the hood, each facade calls `app()` — a **task-local** that holds the booted `Application`. After `Application::boot()`, all facades are available without passing any state through handler signatures. Use `with_app()` (or `#[ravel::test]`) to bind the application to the current task.
