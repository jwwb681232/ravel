# Requests

Ravel provides several ways to access incoming HTTP request data: the `request()` facade for quick access, the `RavelRequest` extractor for the full request object, and `FormRequest` for validated JSON bodies.

## The `request()` Facade

The `request()` facade provides convenient static access to the current HTTP request using a task-local context:

```rust
use ravel_facades::{Route, request};

async fn search() -> String {
    let query: Option<String> = request().query("q");
    let page: Option<u32> = request().query("page");

    match query {
        Some(q) => format!("Searching for {q} on page {}", page.unwrap_or(1)),
        None => "Please provide a search query".to_string(),
    }
}

Route::get("/search", search);
```

### Available Methods

| Method | Description |
|--------|-------------|
| `request().query::<T>(key)` | Get a query-string parameter, deserialized to type `T` |
| `request().header(name)` | Get a request header value |
| `request().path()` | Get the URI path |
| `request().method()` | Get the HTTP method |
| `request().wants_json()` | Check if the client accepts JSON |

```rust
async fn handle_request() -> String {
    // Query parameters
    let name: Option<String> = request().query("name");
    let age: Option<u32> = request().query("age");

    // Headers
    let user_agent = request().header("user-agent").unwrap_or_default();

    // Request metadata
    let method = request().method().unwrap_or_default();
    let path = request().path().unwrap_or_default();

    // Content negotiation
    if request().wants_json() {
        return format!("{{\"path\": \"{path}\"}}");
    }

    format!("{method} {path} | UA: {user_agent}")
}
```

## RavelRequest Extractor

For direct access to the full request object (including headers map and raw query string), use `RavelRequest` as an Axum extractor:

```rust
use ravel_http::request::RavelRequest;
use axum::response::IntoResponse;
use ravel_facades::Route;

async fn inspect(req: RavelRequest) -> impl IntoResponse {
    let method = req.method();
    let path = req.path();
    let content_type = req.content_type().unwrap_or("unknown");
    let wants_json = req.wants_json();
    let all_queries = req.queries();

    format!("{method} {path} | Content-Type: {content_type}")
}

Route::get("/inspect", inspect);
```

`RavelRequest` implements `FromRequest`, so it can be combined with other extractors:

```rust
use axum::extract::Path;
use ravel_http::request::RavelRequest;

async fn show_user(Path(id): Path<u32>, req: RavelRequest) -> String {
    let token = req.header("authorization").unwrap_or("none");
    format!("User {id}, token: {token}")
}
```

## FormRequest & Validated<T>

For JSON APIs, define a request struct with validation rules by implementing the `FormRequest` trait. Use the `Validated<T>` extractor to automatically parse and validate the JSON body:

```rust
use ravel_http::form_request::{FormRequest, Validated};
use ravel_http::validation::{FieldRule, Rule};
use ravel_facades::Route;
use ravel_http::error::RavelError;
use axum::response::IntoResponse;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct CreateUserRequest {
    name: String,
    email: String,
    age: Option<u32>,
}

impl FormRequest for CreateUserRequest {
    fn rules() -> Vec<FieldRule> {
        vec![
            FieldRule::new("name", vec![Rule::Required, Rule::Min(2)]),
            FieldRule::new("email", vec![Rule::Required, Rule::Email]),
            FieldRule::new("age", vec![Rule::Min(18), Rule::Max(120)]),
        ]
    }

    fn messages() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("name.required".into(), "Please enter your name".into());
        m.insert("email.email".into(), "Enter a valid email address".into());
        m
    }

    fn authorize(&self) -> bool {
        // Custom access control — return false to abort with 403
        true
    }
}

async fn store_user(
    Validated(req): Validated<CreateUserRequest>,
) -> Result<impl IntoResponse, RavelError> {
    // req is guaranteed to be valid here
    Ok((axum::http::StatusCode::CREATED,
        serde_json::json!({ "message": format!("Welcome, {}!", req.name) })))
}

Route::post("/users", store_user);
```

When validation fails, the handler automatically returns a 422 response:

```json
{
    "message": "Validation failed",
    "errors": {
        "name": ["Please enter your name"],
        "email": ["Enter a valid email address"]
    }
}
```

## File Uploads

Use the `UploadedFile` extractor for multipart file uploads:

```rust
use ravel_http::upload::UploadedFile;
use axum::response::IntoResponse;
use ravel_facades::Route;

async fn upload_avatar(file: UploadedFile) -> impl IntoResponse {
    let original = file.original_name.as_deref().unwrap_or("unknown");
    let size = file.size;

    // Store the file to a subdirectory
    let path = file.store("avatars").await.unwrap();

    format!("Saved {original} ({size} bytes) to {path:?}")
}

Route::post("/upload", upload_avatar);
```

The `UploadedFile` struct exposes:

| Field/Method | Description |
|-------------|-------------|
| `original_name` | The file name sent by the client |
| `content_type` | Detected MIME type |
| `size` | File size in bytes |
| `data` | Raw file bytes |
| `store(dir)` | Save to a subdirectory under storage root |
| `text()` | Get content as UTF-8 string |
| `bytes()` | Get raw byte slice |

## Complete Example: Search Endpoint

```rust
use ravel_facades::{Route, request};
use ravel_http::error::RavelError;
use axum::response::IntoResponse;

async fn search_posts() -> Result<impl IntoResponse, RavelError> {
    let query: Option<String> = request().query("q");
    let page: u32 = request().query("page").unwrap_or(1);
    let per_page: u32 = request().query("per_page").unwrap_or(20);

    let q = query.ok_or_else(|| {
        RavelError::bad_request("Missing search query 'q'")
    })?;

    let results = perform_search(&q, page, per_page).await;

    Ok(serde_json::json!({
        "query": q,
        "page": page,
        "results": results,
    }))
}

async fn perform_search(query: &str, page: u32, per_page: u32) -> Vec<String> {
    // Simulated search logic
    vec![format!("Result 1 for {query}"), format!("Result 2 for {query}")]
}

Route::get("/posts/search", search_posts);
```
