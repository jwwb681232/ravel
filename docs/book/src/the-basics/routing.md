# Routing

Ravel's router is built on top of Axum and provides a Laravel-inspired API for defining HTTP endpoints. Routes are registered using the `Route` facade inside a service provider.

## Basic Routes

The `Route` facade exposes static methods for each HTTP verb. Handlers can be either closures or function pointers.

```rust
use ravel_facades::Route;

// GET /
Route::get("/", || async { "Hello, Ravel!" });

// POST /posts
Route::post("/posts", || async { "Post created" });

// PUT /posts/1
Route::put("/posts/{id}", || async { "Post updated" });

// DELETE /posts/1
Route::delete("/posts/{id}", || async { "Post deleted" });

// PATCH /posts/1
Route::patch("/posts/{id}", || async { "Post patched" });
```

## Handler Functions

For larger applications, extract handlers into separate functions:

```rust
use axum::response::IntoResponse;
use axum::extract::Path;
use ravel_http::Json;
use ravel_facades::Route;

async fn list_posts() -> impl IntoResponse {
    Json(serde_json::json!([{ "id": 1, "title": "Hello" }]))
}

async fn show_post(Path(id): Path<u32>) -> impl IntoResponse {
    format!("Post {id}")
}

async fn create_post() -> impl IntoResponse {
    (axum::http::StatusCode::CREATED, "created")
}

Route::get("/posts", list_posts);
Route::get("/posts/{id}", show_post);
Route::post("/posts", create_post);
```

## Route Parameters

Use Axum's `Path` extractor to capture URL segments. Axum uses `{param}` syntax for dynamic segments:

```rust
use axum::extract::Path;

async fn show_user(Path(id): Path<u32>) -> String {
    format!("User {id}")
}

async fn show_post_comment(
    Path((post_id, comment_id)): Path<(u32, u32)>,
) -> String {
    format!("Comment {comment_id} on post {post_id}")
}

Route::get("/users/{id}", show_user);
Route::get("/posts/{post_id}/comments/{comment_id}", show_post_comment);
```

## Route Groups

Group routes under a shared prefix using the `group` method. Nested groups are fully supported:

```rust
use ravel_facades::Route;

Route::group("/api", || {
    Route::get("/status", || async { "OK" });

    Route::group("/v1", || {
        Route::get("/users", list_users);
        Route::post("/users", create_user);
        Route::get("/users/{id}", show_user);
    });
});

// Registers these routes:
//   GET  /api/status
//   GET  /api/v1/users
//   POST /api/v1/users
//   GET  /api/v1/users/{id}
```

## Middleware

Attach middleware to all routes registered after the `middleware` call. Middleware is applied in stack order — the first attached runs outermost:

```rust
use ravel_http::middleware::{log_requests, error_handler};
use ravel_facades::Route;

// Apply middleware to all routes below
Route::middleware(log_requests);
Route::middleware(error_handler);

Route::get("/", home);
Route::get("/about", about);

Route::group("/admin", || {
    Route::get("/dashboard", admin_dashboard);
});
```

For CORS configuration and Bearer token auth, use the middleware helpers:

```rust
use ravel_http::middleware::{CorsConfig, BearerAuth};

// CORS middleware
let cors = CorsConfig::new()
    .allow_origin("https://example.com")
    .allow_methods("GET, POST")
    .allow_headers("Content-Type, Authorization");

Route::middleware(cors.middleware());

// Bearer token auth
let auth = BearerAuth::new(|token: String| async move {
    if token == "secret-token" {
        Some("user-1".to_string())
    } else {
        None
    }
});

Route::group("/api", || {
    Route::middleware(auth.middleware());
    Route::get("/profile", profile);
});
```

## Named Routes

The Route facade supports naming routes by convention. While there is no dedicated `name()` method on the facade yet, you can define route names as constants for use in redirects and URL generation:

```rust
// Define route name constants
const ROUTE_HOME: &str = "/";
const ROUTE_POSTS_INDEX: &str = "/posts";
const ROUTE_POSTS_SHOW: &str = "/posts/{id}";

Route::get(ROUTE_HOME, home);
Route::get(ROUTE_POSTS_INDEX, list_posts);
Route::get(ROUTE_POSTS_SHOW, show_post);
```

## Building the Router

After registering all routes, call `Route::build()` to produce an `axum::Router` that can be served:

```rust
use ravel_facades::Route;

// Register routes...

let router = Route::build();

// Pass to the server
ravel_http::server::serve(router, "127.0.0.1:3000").await?;
```

## Complete Example: Blog Routes

Here is a complete example showing public and admin blog routes:

```rust
use ravel_facades::Route;
use ravel_http::middleware::{log_requests, BearerAuth};
use axum::response::IntoResponse;
use axum::extract::Path;

// ── Public routes ────────────────────────────────────────

async fn home() -> impl IntoResponse {
    "Welcome to the blog"
}

async fn list_posts() -> impl IntoResponse {
    "Listing all posts"
}

async fn show_post(Path(slug): Path<String>) -> impl IntoResponse {
    format!("Showing post: {slug}")
}

// ── Admin routes ─────────────────────────────────────────

async fn admin_dashboard() -> impl IntoResponse {
    "Admin dashboard"
}

async fn admin_create_post() -> impl IntoResponse {
    "Post created"
}

async fn admin_edit_post(Path(id): Path<u32>) -> impl IntoResponse {
    format!("Editing post {id}")
}

// ── Route registration ───────────────────────────────────

Route::get("/", home);
Route::get("/posts", list_posts);
Route::get("/posts/{slug}", show_post);

Route::group("/admin", || {
    let admin_auth = BearerAuth::new(|token: String| async move {
        match token.as_str() {
            "admin-token" => Some(1u32),
            _ => None,
        }
    });

    Route::middleware(admin_auth.middleware());
    Route::middleware(log_requests);

    Route::get("/dashboard", admin_dashboard);
    Route::post("/posts", admin_create_post);
    Route::put("/posts/{id}", admin_edit_post);
});

// Build the final router
let router = Route::build();
```
