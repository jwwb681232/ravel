# Rate Limiting

Ravel's `RateLimiter` provides **in-memory sliding-window** rate limiting, ideal for protecting endpoints like login forms or public APIs from abuse.

## Quick Start

```rust
use ravel_http::rate_limit::RateLimiter;
use ravel_facades::Route;

// Allow 60 requests per minute
let limiter = RateLimiter::per_minute(60);

Route::group("/api", || {
    Route::get("/users", list_users);
    Route::get("/posts", list_posts);
});
Route::middleware(limiter.middleware());

let router = Route::build();
```

## Creating a Rate Limiter

Three constructors are available:

```rust
use ravel_http::rate_limit::RateLimiter;
use std::time::Duration;

// 60 requests per minute
let lm = RateLimiter::per_minute(60);

// 1000 requests per hour
let lm = RateLimiter::per_hour(1000);

// Custom window: 10 requests per 30 seconds
let lm = RateLimiter::new(10, Duration::from_secs(30));
```

## Client Identification

The limiter identifies clients by IP address. It checks the `X-Forwarded-For` header first (for reverse proxy setups) and falls back to `127.0.0.1` if no header is present:

```rust
// The middleware internally does:
let client_ip = req
    .headers()
    .get("x-forwarded-for")
    .and_then(|v| v.to_str().ok())
    .unwrap_or("127.0.0.1");
```

Each IP gets an independent counter and window.

## Response Headers

Every response includes rate-limit metadata:

```
X-RateLimit-Limit: 60
X-RateLimit-Remaining: 42
X-RateLimit-Reset: 18
```

When the limit is exceeded, the middleware returns **429 Too Many Requests**:

```json
{
    "message": "Too Many Requests",
    "status": 429
}
```

## Example: Limiting Login Attempts

```rust
use ravel_http::rate_limit::RateLimiter;
use ravel_facades::{Route, Auth, redirect};
use ravel_http::error::RavelError;

fn routes() {
    // Apply a strict 5-per-minute limit to the login endpoint
    let login_limiter = RateLimiter::per_minute(5);

    Route::get("/login", || async { "Login form" });
    Route::post("/login", login_handler);
    Route::middleware(login_limiter.middleware());
}

async fn login_handler(
    axum::Form(form): axum::Form<LoginForm>,
) -> Result<impl axum::response::IntoResponse, RavelError> {
    // If we get here, the rate limit has passed
    let user = find_user(&form.email).await?;
    // ... authenticate ...
    Auth::login(&user.id);
    Ok(redirect("/dashboard"))
}
```

## API Reference

| Method | Description |
|--------|-------------|
| `RateLimiter::new(max, window)` | Custom limit and time window |
| `RateLimiter::per_minute(max)` | Limit per 60-second window |
| `RateLimiter::per_hour(max)` | Limit per 3600-second window |
| `middleware()` | Returns an Axum middleware layer |
| `check(key)` | Manually check if `key` is allowed |

## Limitations

The current implementation stores state in a standard `HashMap` behind a `Mutex`. This is suitable for single-process deployments but does not work across multiple server instances. Redis and database-backed drivers are planned for distributed rate limiting.

For production deployments behind a load balancer, ensure `X-Forwarded-For` is properly set by your reverse proxy so client IPs are correctly identified.
