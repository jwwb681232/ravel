# Helpers

Ravel provides a set of standalone utility functions that are useful across your
application. They are re-exported from `ravel_facades` and require no special setup.

## Environment Variables

```rust
use ravel_facades::{env, env_or};

// Read an env var — returns Option<String>
let db_url: Option<String> = env("DATABASE_URL");

// Read with a fallback default
let port: String = env_or("APP_PORT", "3000");
```

These are thin wrappers around `std::env::var`. They are especially useful in config files
and service providers where you need to read environment values before the config repository
is loaded.

## Timestamp

```rust
use ravel_facades::now;
use chrono::Utc;

let timestamp = now();   // chrono::DateTime<Utc>
println!("Current time: {timestamp}");
```

Returns the current UTC time via `chrono::Utc::now()`.

## Path Helpers

Helpers for building absolute paths relative to the project root:

```rust
use ravel_facades::{config_path, database_path, storage_path};

// Points to <project_root>/config/app.toml
let cfg = config_path("app.toml");

// Points to <project_root>/database/migrations/
let migrations = database_path("migrations");

// Points to <project_root>/storage/logs/app.log
let log_file = storage_path("logs/app.log");
```

These resolve from the current working directory at first invocation and cache the result.

## HTTP Response Helpers

The `redirect`, `back`, and `abort` helpers are designed for use in handlers.

### Redirect

```rust
use ravel_facades::redirect;
use axum::response::IntoResponse;

async fn login() -> impl IntoResponse {
    redirect("/dashboard")   // 302 Found
}
```

### Back

Redirect to the previous page, falling back to `/`:

```rust
use ravel_facades::back;

async fn submit_form() -> impl IntoResponse {
    back()   // 302 to Referer or /
}
```

### Abort

Return an HTTP error response directly:

```rust
use ravel_facades::abort;
use ravel_http::error::RavelError;

fn find_user(id: u32) -> Result<User, RavelError> {
    User::find(id).ok_or_else(|| abort(404, "User not found"))
}

async fn admin_only() -> Result<impl IntoResponse, RavelError> {
    if !is_admin() {
        return Err(abort(403, "Forbidden"));
    }
    Ok(response().json(data)?)
}
```

Supported status codes: `400`, `401`, `403`, `404`, `422`. Other codes produce an
`Internal` error.

## Example: Using Helpers in a Handler

```rust
use ravel_facades::{env, redirect, abort, back, storage_path, now};
use ravel_http::error::RavelError;
use axum::response::IntoResponse;

async fn upload_avatar(file: Upload) -> Result<impl IntoResponse, RavelError> {
    // Guard: feature flag via env
    if !env("FEATURE_AVATARS").is_some_and(|v| v == "1") {
        return Err(abort(404, "Not found"));
    }

    // Guard: max file size from env
    let max_bytes: usize = env_or("MAX_UPLOAD_BYTES", "2097152").parse().unwrap_or(2097152);
    if file.size > max_bytes {
        return Err(abort(422, "File too large"));
    }

    // Save to storage
    let dest = storage_path(&format!("avatars/{}.png", now().timestamp()));
    file.save(&dest).await?;

    Ok(redirect("/profile"))
}

async fn delete_avatar() -> impl IntoResponse {
    // Perform cleanup ...
    back()  // return to the previous page
}
```

## API Reference

| Function | Description |
|----------|-------------|
| `env(key)` | Read environment variable, returns `Option<String>` |
| `env_or(key, default)` | Read env var with a fallback default |
| `now()` | Current UTC timestamp (`chrono::DateTime<Utc>`) |
| `config_path(file)` | Path under `config/` |
| `database_path(file)` | Path under `database/` |
| `storage_path(file)` | Path under `storage/` |
| `redirect(url)` | 302 redirect response |
| `back()` | 302 redirect to `Referer` (falls back to `/`) |
| `abort(status, message)` | HTTP error response as `RavelError` |
