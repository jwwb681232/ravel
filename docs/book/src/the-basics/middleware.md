# Middleware

Middleware provides a convenient mechanism for filtering HTTP requests entering your application. Each middleware runs as an async function wrapping the request/response cycle.

## How Middleware Works

A middleware function takes an incoming request, performs some logic, and passes control to the next middleware (or the final handler) via `next.run(req)`. It can modify the request before passing it along, or modify the response on the way back out.

```rust
use axum::extract::Request;
use ravel_http::middleware::Next;
use axum::response::Response;

async fn my_middleware(req: Request, next: Next) -> Response {
    // Pre-processing: inspect or modify the request
    let response = next.run(req).await;
    // Post-processing: inspect or modify the response
    response
}
```

## Attaching Middleware

Use `Route::middleware(fn)` to attach middleware to all routes registered after the call:

```rust
use ravel_facades::Route;
use ravel_http::middleware::log_requests;

// All routes below this line run through log_requests
Route::middleware(log_requests);

Route::get("/", home);
Route::get("/about", about);
```

Middleware is applied in **stack order** — the first middleware you attach runs outermost (receives the raw request first, sends the response out last):

```rust
Route::middleware(outer_middleware);  // runs first
Route::middleware(inner_middleware); // runs second, closer to the handler
Route::get("/", handler);            // handler runs last (innermost)
```

## Built-in Middleware

### Request Logger

Logs the HTTP method, path, and response status for every request:

```rust
use ravel_http::middleware::log_requests;

Route::middleware(log_requests);
Route::get("/", home);
```

Output:

```
INFO GET / -> 200
INFO POST /posts -> 201
```

### Error Handler

Catches panics and unhandled errors, returning a 500 JSON response instead of crashing:

```rust
use ravel_http::middleware::error_handler;

Route::middleware(error_handler);
Route::get("/", safe_handler);
```

### CORS Configuration

Configure cross-origin resource sharing:

```rust
use ravel_http::middleware::CorsConfig;

let cors = CorsConfig::new()
    .allow_origin("https://myapp.com")
    .allow_methods("GET, POST, PUT, DELETE, PATCH")
    .allow_headers("Content-Type, Authorization, X-Requested-With")
    .max_age(86400);

Route::middleware(cors.middleware());
Route::get("/api/data", api_data);
```

The default `CorsConfig` allows all origins (`*`) with standard HTTP methods. Use the builder methods to tighten security for production.

### Bearer Token Authentication

Validates an `Authorization: Bearer <token>` header using a custom validator function. On success, the validated user ID is stored in request extensions:

```rust
use ravel_http::middleware::BearerAuth;

// The validator receives the token string and returns Some(user_id) or None
let auth = BearerAuth::new(|token: String| async move {
    // Look up the token in a database or validate with an external service
    match token.as_str() {
        "sk-live-abc123" => Some(42u32),
        _ => None,
    }
});

Route::middleware(auth.middleware());

Route::get("/protected", protected_handler);
```

On failure, the middleware returns a 401 JSON response:

```json
{
    "message": "Invalid token",
    "status": 401
}
```

## Writing Custom Middleware

Any async function with the signature `fn(Request, Next) -> Response` can serve as middleware. Here is a timing middleware that measures latency:

```rust
use axum::extract::Request;
use ravel_http::middleware::Next;
use axum::response::Response;
use tracing::info;
use std::time::Instant;

async fn timing_middleware(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    let response = next.run(req).await;

    let elapsed = start.elapsed();
    info!("{method} {path} took {elapsed:?}");
    response
}

// Register it
Route::middleware(timing_middleware);
Route::get("/slow", slow_endpoint);
```

Here is a middleware that adds a custom security header:

```rust
use axum::extract::Request;
use ravel_http::middleware::Next;
use axum::response::Response;

async fn security_headers(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    response
        .headers_mut()
        .insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    response
        .headers_mut()
        .insert("X-Frame-Options", "DENY".parse().unwrap());
    response
}

Route::middleware(security_headers);
```

## Middleware Ordering with Groups

Combine middleware and route groups for fine-grained control:

```rust
use ravel_http::middleware::log_requests;
use ravel_facades::Route;

// Global middleware for all routes
Route::middleware(log_requests);

// Public routes — no auth
Route::get("/", home);
Route::get("/login", login_form);

// Admin routes — auth required
Route::group("/admin", || {
    Route::middleware(admin_auth);
    Route::get("/dashboard", admin_dashboard);
    Route::get("/users", admin_users);
});
```

## Complete Example: CORS Configuration

```rust
use ravel_http::middleware::{CorsConfig, log_requests};
use ravel_facades::Route;

let cors = CorsConfig::new()
    .allow_origin("http://localhost:5173")
    .allow_methods("GET, POST, PUT, DELETE")
    .allow_headers("Content-Type, Authorization")
    .max_age(3600);

Route::middleware(log_requests);
Route::middleware(cors.middleware());

Route::get("/api/posts", list_posts);
Route::post("/api/posts", create_post);
Route::put("/api/posts/{id}", update_post);
Route::delete("/api/posts/{id}", delete_post);

let router = Route::build();
```
