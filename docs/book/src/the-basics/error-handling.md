# Error Handling

Ravel provides a unified error handling system built around the `RavelError` enum. It implements `IntoResponse`, so handlers can return `Result<impl IntoResponse, RavelError>` and use the `?` operator for clean error propagation.

## The RavelError Enum

```rust
use ravel_http::error::RavelError;

pub enum RavelError {
    NotFound(String),          // 404
    BadRequest(String),        // 400
    Unauthorized(String),      // 401
    Forbidden(String),         // 403
    ValidationError(HashMap<String, Vec<String>>),  // 422
    Internal(anyhow::Error),   // 500
}
```

### Constructor Methods

Each variant has a convenience constructor:

```rust
RavelError::not_found("User not found");
RavelError::bad_request("Invalid payload");
RavelError::unauthorized("Missing API key");
RavelError::forbidden("Insufficient permissions");
RavelError::validation_error(errors_map);
```

## Using `?` in Handlers

Because `RavelError` implements `IntoResponse`, you can propagate errors naturally:

```rust
use ravel_http::error::RavelError;
use ravel_facades::Route;
use axum::response::IntoResponse;

async fn find_user(id: u32) -> Result<String, RavelError> {
    // Simulate a database lookup
    match id {
        1 => Ok("Alice".to_string()),
        2 => Ok("Bob".to_string()),
        _ => Err(RavelError::not_found(format!("User {id} not found"))),
    }
}

async fn show_user(id: u32) -> Result<impl IntoResponse, RavelError> {
    let name = find_user(id).await?; // Propagates 404 automatically
    Ok(format!("Hello, {name}!"))
}

Route::get("/users/{id}", show_user);
```

When `RavelError` is returned, Axum converts it to a JSON error response:

```json
{
    "message": "User 42 not found",
    "status": 404
}
```

## The `abort()` Helper

Use `abort(status, message)` for quick error responses:

```rust
use ravel_facades::{abort, Route};

// Returns a 404 RavelError
Route::get("/users/{id}", |id: u32| async move {
    let user = find_user(id).await.ok_or_else(|| abort(404, "Not found"))?;
    Ok::<_, RavelError>(format!("User: {user}"))
});
```

The `abort()` function maps status codes:

| Status Code | Resulting Error |
|-------------|----------------|
| 400 | `RavelError::BadRequest` |
| 401 | `RavelError::Unauthorized` |
| 403 | `RavelError::Forbidden` |
| 404 | `RavelError::NotFound` |
| 422 | `RavelError::ValidationError` |
| Other | `RavelError::Internal` |

## From<anyhow::Error> Conversion

`RavelError` implements `From<anyhow::Error>`, so you can use `?` on any `anyhow::Result` inside your handlers:

```rust
use ravel_http::error::RavelError;
use axum::response::IntoResponse;

async fn read_config() -> Result<String, anyhow::Error> {
    let content = tokio::fs::read_to_string("config/app.toml").await?;
    Ok(content)
}

async fn show_config() -> Result<impl IntoResponse, RavelError> {
    let config = read_config().await?; // anyhow::Error → RavelError::Internal
    Ok(config)
}
```

Any `anyhow::Error` is automatically converted to a 500 Internal Server Error. The error details are logged via `tracing` but **not** exposed in the response body for security reasons:

```json
{
    "message": "Internal server error",
    "status": 500
}
```

## JSON Error Response Format

All errors produce consistent JSON bodies:

| Status | Response Body |
|--------|---------------|
| 400 | `{"message": "...", "status": 400}` |
| 401 | `{"message": "...", "status": 401}` |
| 403 | `{"message": "...", "status": 403}` |
| 404 | `{"message": "...", "status": 404}` |
| 422 | `{"message": "Validation failed", "errors": {...}, "status": 422}` |
| 500 | `{"message": "Internal server error", "status": 500}` |

## Validation Errors

When using `FormRequest` with `Validated<T>`, failed validation produces a 422 error with field-level messages:

```rust
use ravel_http::form_request::{FormRequest, Validated};
use ravel_http::validation::{FieldRule, Rule};
use ravel_http::error::RavelError;
use axum::response::IntoResponse;
use serde::Deserialize;

#[derive(Deserialize)]
struct CreateUserRequest {
    name: String,
    email: String,
}

impl FormRequest for CreateUserRequest {
    fn rules() -> Vec<FieldRule> {
        vec![
            FieldRule::new("name", vec![Rule::Required]),
            FieldRule::new("email", vec![Rule::Required, Rule::Email]),
        ]
    }
}

async fn store(
    Validated(req): Validated<CreateUserRequest>,
) -> Result<impl IntoResponse, RavelError> {
    // Automatically returns 422 if validation fails
    Ok(serde_json::json!({ "created": true }))
}
```

## Custom Error Responses

For application-specific error types, implement `IntoResponse` directly:

```rust
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum::http::StatusCode;
use serde_json::json;

enum AppError {
    PaymentRequired(String),
    RateLimited { retry_after: u64 },
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::PaymentRequired(msg) => {
                (StatusCode::PAYMENT_REQUIRED, Json(json!({
                    "message": msg,
                    "status": 402,
                }))).into_response()
            }
            AppError::RateLimited { retry_after } => {
                (StatusCode::TOO_MANY_REQUESTS, Json(json!({
                    "message": "Rate limit exceeded",
                    "retry_after": retry_after,
                    "status": 429,
                }))).into_response()
            }
        }
    }
}

async fn premium_content() -> Result<impl IntoResponse, AppError> {
    Err(AppError::PaymentRequired("Subscription required".into()))
}
```

## Complete Example: User Controller

```rust
use ravel_http::error::RavelError;
use ravel_facades::{Route, abort, response};
use axum::response::IntoResponse;
use axum::extract::Path;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize)]
struct User {
    id: u32,
    name: String,
    email: String,
}

// Simulated database
fn db_find_user(id: u32) -> Option<User> {
    match id {
        1 => Some(User { id: 1, name: "Alice".into(), email: "alice@example.com".into() }),
        2 => Some(User { id: 2, name: "Bob".into(), email: "bob@example.com".into() }),
        _ => None,
    }
}

fn db_find_user_by_email(email: &str) -> Option<User> {
    if email == "alice@example.com" {
        Some(User { id: 1, name: "Alice".into(), email: email.into() })
    } else {
        None
    }
}

async fn show(Path(id): Path<u32>) -> Result<impl IntoResponse, RavelError> {
    let user = db_find_user(id)
        .ok_or_else(|| RavelError::not_found(format!("User {id} not found")))?;

    Ok(response().json(&user).unwrap())
}

#[derive(Deserialize)]
struct UpdateUserPayload {
    name: Option<String>,
    email: Option<String>,
}

async fn update(
    Path(id): Path<u32>,
    Json(payload): Json<UpdateUserPayload>,
) -> Result<impl IntoResponse, RavelError> {
    // Check user exists
    let _user = db_find_user(id)
        .ok_or_else(|| RavelError::not_found(format!("User {id} not found")))?;

    // Validate email uniqueness if provided
    if let Some(ref email) = payload.email {
        if db_find_user_by_email(email).is_some() {
            let mut errors = HashMap::new();
            errors.insert("email".into(), vec!["Email already taken".into()]);
            return Err(RavelError::validation_error(errors));
        }
    }

    Ok(response().json(serde_json::json!({
        "updated": true,
        "id": id,
    })).unwrap())
}

async fn delete(Path(id): Path<u32>) -> Result<impl IntoResponse, RavelError> {
    // Authorization check via header
    let token = ravel_facades::request().header("authorization");
    match token {
        Some(t) if t == "Bearer admin-token" => {}
        _ => return Err(RavelError::unauthorized("Admin token required")),
    }

    let _user = db_find_user(id)
        .ok_or_else(|| RavelError::not_found(format!("User {id} not found")))?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn list() -> Result<impl IntoResponse, RavelError> {
    let users = vec![
        User { id: 1, name: "Alice".into(), email: "alice@example.com".into() },
        User { id: 2, name: "Bob".into(), email: "bob@example.com".into() },
    ];
    Ok(response().json(&users).unwrap())
}

// Route registration
Route::get("/users", list);
Route::get("/users/{id}", show);
Route::put("/users/{id}", update);
Route::delete("/users/{id}", delete);
```
