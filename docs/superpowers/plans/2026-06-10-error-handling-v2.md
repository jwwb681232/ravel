# Error Handling Architecture v2 Implementation Plan

> **For agentic workers:** Execute this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `ravel-error` protocol crate + `HttpError` trait bridge so typed errors auto-convert to HTTP responses.

**Architecture:** New `ravel-error` crate defines `HttpError` trait + `ErrorKind` enum. `ravel-eloquent` implements `HttpError`. `ravel-http` adds `From` bridges that auto-convert any `HttpError` into `RavelError` → HTTP response.

**Tech Stack:** `http` crate (StatusCode), `thiserror` (existing), `anyhow` (existing)

---

### Task 1: Create `ravel-error` crate

**Files:**
- Create: `crates/ravel-error/Cargo.toml`
- Create: `crates/ravel-error/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "ravel-error"
version = "0.1.0"
edition = "2024"
description = "Shared error protocol for the Ravel framework"
license = "MIT"

[dependencies]
http = "1"
```

- [ ] **Step 2: Create src/lib.rs**

```rust
use http::StatusCode;

/// Classification of an error for HTTP response mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    NotFound,
    BadRequest,
    Unauthorized,
    Forbidden,
    Validation,
    Conflict,
    TooManyRequests,
    Internal,
}

impl From<ErrorKind> for StatusCode {
    fn from(k: ErrorKind) -> Self {
        match k {
            ErrorKind::NotFound => StatusCode::NOT_FOUND,
            ErrorKind::BadRequest => StatusCode::BAD_REQUEST,
            ErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorKind::Forbidden => StatusCode::FORBIDDEN,
            ErrorKind::Validation => StatusCode::UNPROCESSABLE_ENTITY,
            ErrorKind::Conflict => StatusCode::CONFLICT,
            ErrorKind::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            ErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// Any error implementing this trait is automatically convertible
/// into an HTTP response by ravel-http's ErrorBridge.
pub trait HttpError: std::error::Error + Send + Sync + 'static {
    /// Which HTTP status code this error should produce.
    fn status_code(&self) -> StatusCode {
        self.kind().into()
    }

    /// The error's classification.
    fn kind(&self) -> ErrorKind {
        ErrorKind::Internal
    }
}
```

- [ ] **Step 3: Register in workspace**

In root `Cargo.toml`, add `"crates/ravel-error"` to the `members` list, after `"crates/ravel-eloquent-macros"`.

- [ ] **Step 4: Verify compile**

```bash
cargo check -p ravel-error
```

Expected: `Finished dev profile` with 0 errors.

- [ ] **Step 5: Commit**

```bash
git add crates/ravel-error/ Cargo.toml
git commit -m "feat(error): add ravel-error protocol crate with HttpError trait"
```

---

### Task 2: ravel-eloquent implements HttpError

**Files:**
- Modify: `crates/ravel-eloquent/Cargo.toml`
- Modify: `crates/ravel-eloquent/src/error.rs`

- [ ] **Step 1: Add ravel-error dependency**

In `crates/ravel-eloquent/Cargo.toml`, add under `[dependencies]`:

```toml
ravel-error = { path = "../ravel-error" }
```

- [ ] **Step 2: Add ValidationError variant and HttpError impl**

In `crates/ravel-eloquent/src/error.rs`, after the existing enum definition, add:

```rust
use ravel_error::{ErrorKind, HttpError};
use std::collections::HashMap;

// Add ValidationError variant to the existing RavelEloquentError enum:
//   #[error("validation failed: {errors:?}")]
//   ValidationError { errors: HashMap<String, Vec<String>> },

impl HttpError for RavelEloquentError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::RecordNotFound { .. } => ErrorKind::NotFound,
            Self::ValidationError { .. } => ErrorKind::Validation,
            Self::InvalidColumn { .. }
            | Self::UpdateWithoutId
            | Self::InsertWithId => ErrorKind::BadRequest,
            Self::Database(_) | Self::Serialization(_) | Self::Other(_) => ErrorKind::Internal,
        }
    }
}
```

- [ ] **Step 3: Verify compile + tests**

```bash
cargo check -p ravel-eloquent
cargo test -p ravel-eloquent
```

Expected: 60 tests pass, 0 failures.

- [ ] **Step 4: Commit**

```bash
git add crates/ravel-eloquent/
git commit -m "feat(eloquent): impl HttpError for RavelEloquentError"
```

---

### Task 3: ravel-http ErrorBridge

**Files:**
- Modify: `crates/ravel-http/Cargo.toml`
- Modify: `crates/ravel-http/src/error.rs`

- [ ] **Step 1: Add ravel-error dependency**

In `crates/ravel-http/Cargo.toml`, add under `[dependencies]`:

```toml
ravel-error = { path = "../ravel-error" }
```

- [ ] **Step 2: Add From<E: HttpError> for RavelError bridge**

In `crates/ravel-http/src/error.rs`, add after the existing `RavelError` impl block:

```rust
use ravel_error::{ErrorKind, HttpError};

impl<E: HttpError> From<E> for RavelError {
    fn from(e: E) -> Self {
        let msg = e.to_string();
        match e.kind() {
            ErrorKind::NotFound => RavelError::NotFound(msg),
            ErrorKind::BadRequest => RavelError::BadRequest(msg),
            ErrorKind::Unauthorized => RavelError::Unauthorized(msg),
            ErrorKind::Forbidden => RavelError::Forbidden(msg),
            ErrorKind::Validation => {
                RavelError::ValidationError(std::collections::HashMap::new())
            }
            ErrorKind::Conflict | ErrorKind::TooManyRequests => RavelError::BadRequest(msg),
            ErrorKind::Internal => {
                tracing::error!(error = %e, "internal error");
                RavelError::Internal(anyhow::anyhow!("{}", e))
            }
        }
    }
}

impl From<anyhow::Error> for RavelError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!(error = ?e, "unexpected error");
        RavelError::Internal(e)
    }
}
```

- [ ] **Step 3: Verify compile**

```bash
cargo check -p ravel-http
```

- [ ] **Step 4: Write integration test**

In `crates/ravel-http/src/error.rs`, add to the existing `#[cfg(test)]` module:

```rust
#[test]
fn eloquent_error_auto_converts_to_404() {
    let err = ravel_eloquent::RavelEloquentError::RecordNotFound {
        table: "users",
        id: "42".into(),
    };
    let ravel_err: RavelError = err.into();
    let response = ravel_err.into_response();
    assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
}

#[test]
fn anyhow_error_converts_to_500() {
    let err = anyhow::anyhow!("something broke");
    let ravel_err: RavelError = err.into();
    let response = ravel_err.into_response();
    assert_eq!(response.status(), http::StatusCode::INTERNAL_SERVER_ERROR);
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p ravel-http
```

Expected: all tests pass including the two new bridge tests.

- [ ] **Step 6: Full workspace check**

```bash
cargo check
cargo test -p ravel-eloquent
```

Expected: workspace compiles, 60 eloquent tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/ravel-http/
git commit -m "feat(http): add ErrorBridge — From<HttpError> and From<anyhow::Error> for RavelError"
```

---

### Completion checklist

- [ ] `cargo check` workspace clean
- [ ] `cargo test -p ravel-error` passes (no tests yet, just compiles)
- [ ] `cargo test -p ravel-eloquent` — 60/60 pass
- [ ] `cargo test -p ravel-http` — all pass including bridge tests
- [ ] User handlers can use `User::find_or_fail(&db, id).await?` directly with `Result<_, RavelError>`
