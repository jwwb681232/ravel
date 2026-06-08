# Responses

Ravel provides a fluent response builder and several helper functions for returning HTTP responses from your handlers.

## The `response()` Builder

Use `response()` to build HTTP responses with a fluent API:

```rust
use ravel_facades::{Route, response};
use axum::response::IntoResponse;

async fn json_endpoint() -> impl IntoResponse {
    response()
        .json(serde_json::json!({"status": "ok", "data": [1, 2, 3]}))
        .unwrap()
}

async fn custom_status() -> impl IntoResponse {
    response()
        .status(axum::http::StatusCode::ACCEPTED)
        .body("Request accepted, processing will continue")
}

async fn with_header() -> impl IntoResponse {
    response()
        .header("X-Custom", "my-value")
        .body("Response with custom header")
}

Route::get("/json", json_endpoint);
Route::post("/accepted", custom_status);
Route::get("/header", with_header);
```

### Builder Methods

| Method | Description |
|--------|-------------|
| `response().json(data)` | JSON response body (returns `Result`) |
| `response().status(code)` | Set HTTP status code |
| `response().body(text)` | Plain text response body |
| `response().header(key, value)` | Add a response header |
| `response().build()` | Materialise into a `Response` |

`ResponseBuilder` implements `IntoResponse`, so you can return it directly from handlers without calling `.build()`.

## Status Code Shortcuts

Use `axum::http::StatusCode` for common HTTP status codes:

```rust
use axum::http::StatusCode;
use axum::response::IntoResponse;

async fn created() -> impl IntoResponse {
    (StatusCode::CREATED, "Resource created")
}

async fn no_content() -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

async fn not_modified() -> impl IntoResponse {
    StatusCode::NOT_MODIFIED
}
```

## Redirects

The `redirect()` function returns a 302 Found redirect. Use `back()` to redirect to the previous page (currently falls back to `/`):

```rust
use ravel_facades::{Route, redirect, back};
use axum::response::IntoResponse;

async fn login_post() -> impl IntoResponse {
    // After login, redirect to dashboard
    redirect("/dashboard")
}

async fn cancel() -> impl IntoResponse {
    // Go back to the previous page
    back()
}

Route::post("/login", login_post);
Route::post("/cancel", cancel);
```

`ResponseBuilder` also provides static methods for redirects:

```rust
use ravel_http::response::ResponseBuilder;

async fn go_home() -> impl IntoResponse {
    ResponseBuilder::redirect("/")
}

async fn permanent_redirect() -> impl IntoResponse {
    ResponseBuilder::redirect_permanent("/new-location")
}
```

## Error Responses with `abort()`

The `abort()` function returns a `RavelError` for common HTTP error status codes:

```rust
use ravel_facades::{abort, Route};
use ravel_http::error::RavelError;
use axum::response::IntoResponse;

async fn find_user(id: u32) -> Result<String, RavelError> {
    Err(abort(404, format!("User {id} not found")))
}

Route::get("/users/{id}", find_user);
```

The `abort()` helper maps status codes to `RavelError` variants:

| Status | Constructor |
|--------|-------------|
| 400 | `RavelError::bad_request(msg)` |
| 401 | `RavelError::unauthorized(msg)` |
| 403 | `RavelError::forbidden(msg)` |
| 404 | `RavelError::not_found(msg)` |
| 422 | `RavelError::validation_error(...)` |

## Using `?` with RavelError

`RavelError` implements `IntoResponse`, so handlers can return `Result<impl IntoResponse, RavelError>` and use the `?` operator for clean error propagation:

```rust
use ravel_facades::{Route, response, abort};
use ravel_http::error::RavelError;
use axum::response::IntoResponse;
use std::collections::HashMap;

async fn get_user(id: u32) -> Result<serde_json::Value, RavelError> {
    // Simulated lookup — returns None for id > 100
    if id > 100 {
        return Err(RavelError::not_found(format!("User {id} not found")));
    }
    Ok(serde_json::json!({"id": id, "name": "Alice"}))
}

async fn show_user(id: u32) -> Result<impl IntoResponse, RavelError> {
    let user = get_user(id).await?; // Propagates 404 automatically

    if request().wants_json() {
        return Ok(response().json(user).unwrap());
    }

    Ok(format!("User: {}", user["name"]))
}

use ravel_facades::request;

Route::get("/users/{id}", show_user);
```

## Complete Example: CRUD Handler

```rust
use ravel_facades::{Route, response, abort, request};
use ravel_http::error::RavelError;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::extract::Path;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::collections::HashMap;

#[derive(Serialize, Clone)]
struct Post {
    id: u32,
    title: String,
    body: String,
}

// In-memory store (for demonstration only)
static POSTS: std::sync::LazyLock<Mutex<HashMap<u32, Post>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

async fn index() -> impl IntoResponse {
    let posts = POSTS.lock().unwrap();
    let items: Vec<&Post> = posts.values().collect();
    response().json(items).unwrap()
}

async fn show(Path(id): Path<u32>) -> Result<impl IntoResponse, RavelError> {
    let posts = POSTS.lock().unwrap();
    match posts.get(&id) {
        Some(post) => Ok(response().json(post).unwrap()),
        None => Err(RavelError::not_found(format!("Post {id} not found"))),
    }
}

#[derive(Deserialize)]
struct CreatePostPayload {
    title: String,
    body: String,
}

async fn store(Json(payload): Json<CreatePostPayload>) -> Result<impl IntoResponse, RavelError> {
    let id = rand::random::<u32>();
    let post = Post {
        id,
        title: payload.title,
        body: payload.body,
    };
    POSTS.lock().unwrap().insert(id, post.clone());

    Ok((StatusCode::CREATED, response().json(post).unwrap()))
}

async fn destroy(Path(id): Path<u32>) -> Result<impl IntoResponse, RavelError> {
    let mut posts = POSTS.lock().unwrap();
    match posts.remove(&id) {
        Some(_) => Ok(StatusCode::NO_CONTENT),
        None => Err(RavelError::not_found(format!("Post {id} not found"))),
    }
}

// Route registration
Route::get("/posts", index);
Route::get("/posts/{id}", show);
Route::post("/posts", store);
Route::delete("/posts/{id}", destroy);
```
