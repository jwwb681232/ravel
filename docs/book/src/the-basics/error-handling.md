# Error Handling

Ravel's error system is built around two complementary layers: the `RavelError` HTTP type for handler responses, and the `HttpError` trait protocol for automatic cross-crate error conversion.

## RavelError

`RavelError` implements `IntoResponse`, so handlers return `Result<impl IntoResponse, RavelError>` and use `?`:

```rust
use ravel_http::error::RavelError;
use axum::response::IntoResponse;

async fn show_user(Path(id): Path<i32>) -> Result<impl IntoResponse, RavelError> {
    let user = find_user(id)
        .ok_or_else(|| RavelError::not_found("User not found"))?;
    Ok(Json(user))
}
```

| Variant | HTTP Status | Factory |
|---------|:----------:|---------|
| `NotFound(msg)` | 404 | `RavelError::not_found(...)` |
| `BadRequest(msg)` | 400 | `RavelError::bad_request(...)` |
| `Unauthorized(msg)` | 401 | `RavelError::unauthorized(...)` |
| `Forbidden(msg)` | 403 | `RavelError::forbidden(...)` |
| `ValidationError(map)` | 422 | `RavelError::validation_error(errors)` |
| `Internal(anyhow::Error)` | 500 | `RavelError::Internal(...)` |

All responses are JSON:

```json
{"message": "User not found", "status": 404}
```

## HttpError Protocol

The `ravel-error` crate defines the `HttpError` trait. Any error type implementing it is **automatically** convertible into `RavelError` — no manual `.map_err()` needed:

```rust
use ravel_error::{ErrorKind, HttpError};
use http::StatusCode;

#[derive(Debug)]
struct PaymentError;

impl std::fmt::Display for PaymentError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "payment declined")
    }
}
impl std::error::Error for PaymentError {}
impl HttpError for PaymentError {
    fn kind(&self) -> ErrorKind { ErrorKind::BadRequest }
}
```

## Eloquent Errors Auto-Convert

`RavelEloquentError` already implements `HttpError`, so all ORM errors map automatically:

| Eloquent Error | HTTP Status |
|----------------|:----------:|
| `RecordNotFound` | 404 |
| `ValidationError` | 422 |
| `InvalidColumn`, `UpdateWithoutId`, `InsertWithId` | 400 |
| `Database`, `Serialization`, `Other` | 500 |

```rust
// Before: manual .map_err()
let user = User::find_or_fail(&db, id)
    .map_err(|e| RavelError::NotFound(e.to_string()))?;

// After: automatic conversion
let user = User::find_or_fail(&db, id).await?;
```

## Custom Errors

Implement `HttpError` on your own types and they gain the same auto-conversion:

```rust
#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("rate limit exceeded")]
    RateLimited,
}

impl HttpError for AppError {
    fn kind(&self) -> ErrorKind {
        match self {
            AppError::RateLimited => ErrorKind::TooManyRequests,
        }
    }
}

// In handlers:
async fn sensitive() -> Result<impl IntoResponse, RavelError> {
    check_limit().map_err(|_| AppError::RateLimited)?; // → 429
    Ok("ok")
}
```

## ErrorKind Reference

| ErrorKind | HTTP Status |
|-----------|:----------:|
| `NotFound` | 404 |
| `BadRequest` | 400 |
| `Unauthorized` | 401 |
| `Forbidden` | 403 |
| `Validation` | 422 |
| `Conflict` | 409 |
| `TooManyRequests` | 429 |
| `Internal` | 500 |
