# Ravel Error Handling Architecture v2

**Date:** 2026-06-10
**Status:** approved

## 1. Problem

Current error handling has three incompatible philosophies across crates:

| Crate | Library | Issue |
|-------|---------|-------|
| ravel-eloquent | thiserror | Only crate with typed errors |
| ravel-http | Hand-written enum | No `From<RavelEloquentError>` bridge |
| ravel-core / db / etc | anyhow | Undifferentiated errors, `ravel-db-core` trait signatures locked to `anyhow::Result` |

User handlers must manually convert every Eloquent error:

```rust
// Current — every call needs .map_err()
let user = User::find_or_fail(&db, id)
    .map_err(|e| RavelError::NotFound(e.to_string()))?;
```

Goal: typed errors auto-convert to HTTP responses. User types `?` and the framework handles the rest.

## 2. Design

### 2.1 New crate: `ravel-error`

A zero-weight protocol crate defining the contract between any subsystem and the HTTP layer.

```rust
// crates/ravel-error/src/lib.rs
use http::StatusCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    NotFound,        // 404
    BadRequest,      // 400
    Unauthorized,    // 401
    Forbidden,       // 403
    Validation,      // 422
    Conflict,        // 409
    TooManyRequests, // 429
    Internal,        // 500
}

impl From<ErrorKind> for StatusCode { /* ... */ }

pub trait HttpError: std::error::Error + Send + Sync + 'static {
    fn status_code(&self) -> StatusCode { self.kind().into() }
    fn kind(&self) -> ErrorKind { ErrorKind::Internal }
}
```

Dependencies: only `http = "1"`. Compiles in under 1 second.

### 2.2 ravel-eloquent: impl HttpError

Add `impl HttpError for RavelEloquentError` mapping each variant to an `ErrorKind`:

| Variant | ErrorKind |
|---------|-----------|
| RecordNotFound | NotFound |
| ValidationError (new) | Validation |
| InvalidColumn, UpdateWithoutId, InsertWithId | BadRequest |
| Database, Serialization, Other | Internal |

Also add `ValidationError { errors: HashMap<String, Vec<String>> }` variant.

### 2.3 ravel-http: ErrorBridge

Two `From` implementations added to `error.rs`:

**Bridge 1 — typed errors:**

```rust
impl<E: HttpError> From<E> for RavelError {
    fn from(e: E) -> Self {
        match e.kind() {
            ErrorKind::NotFound => RavelError::NotFound(e.to_string()),
            ErrorKind::BadRequest => RavelError::BadRequest(e.to_string()),
            ErrorKind::Unauthorized => RavelError::Unauthorized(e.to_string()),
            ErrorKind::Forbidden => RavelError::Forbidden(e.to_string()),
            ErrorKind::Validation => RavelError::ValidationError(HashMap::new()),
            ErrorKind::Conflict | ErrorKind::TooManyRequests => RavelError::BadRequest(e.to_string()),
            ErrorKind::Internal => {
                tracing::error!(error = %e, "internal error");
                RavelError::Internal(anyhow::anyhow!("{}", e))
            }
        }
    }
}
```

**Bridge 2 — anyhow fallback:**

```rust
impl From<anyhow::Error> for RavelError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!(error = ?e, "unexpected error");
        RavelError::Internal(e)
    }
}
```

### 2.4 User experience

After this change, user handlers no longer need manual `.map_err()`:

```rust
// Before
async fn show() -> Result<Json<User>, RavelError> {
    let user = User::find_or_fail(&db, id)
        .map_err(|e| RavelError::NotFound(e.to_string()))?;
    Ok(Json(user))
}

// After — typed errors auto-convert
async fn show() -> Result<Json<User>, RavelError> {
    let user = User::find_or_fail(&db, id).await?;  // RecordNotFound → 404
    Ok(Json(user))
}
```

## 3. Non-goals

- NOT fixing facade `.expect()` panics (separate problem)
- NOT forcing any crate to migrate from anyhow to thiserror
- NOT adding `Conflict`/`TooManyRequests` variants to `RavelError` (ErrorKind reserved, mapped to BadRequest for now)
- NOT touching `ravel-db-core` trait signatures (breaking change, defer to v0.2)

## 4. Migration plan

Four phases, each independently testable:

| Phase | Files | Effect |
|-------|-------|--------|
| 1. ravel-error | 2 new files | Zero impact — pure addition |
| 2. eloquent HttpError | 2 modified | Additive — no behavior change |
| 3. http ErrorBridge | 2 modified | Additive — existing tests still pass |
| 4. User code cleanup | Optional | Delete `.map_err()` boilerplate |

## 5. File manifest

```
crates/ravel-error/              [NEW]
├── Cargo.toml                   # [dependencies] http = "1"
└── src/lib.rs                   # ErrorKind + HttpError (~40 lines)

crates/ravel-eloquent/
├── Cargo.toml                   # +ravel-error dep
└── src/error.rs                 # +impl HttpError + ValidationError (~30 lines)

crates/ravel-http/
├── Cargo.toml                   # +ravel-error dep
└── src/error.rs                 # +2 From impls (~40 lines)

Cargo.toml (workspace)            # +ravel-error member
```

Total: ~120 lines, 3 new files, 4 modified files.

## 6. Verification

- Phase 1: `cargo check -p ravel-error` passes
- Phase 2: `cargo test -p ravel-eloquent` — all 60 tests pass
- Phase 3: `cargo test -p ravel-http` passes + new integration test proving EloquentError → 404 via `IntoResponse`
- End-to-end: `cargo check` workspace clean
