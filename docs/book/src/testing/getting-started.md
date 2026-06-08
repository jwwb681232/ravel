# Testing — Getting Started

Ravel's `TestClient` lets you send HTTP requests to your Axum router and
assert on the response — all in-process, without binding to a port or running a
separate server.

## TestClient

Wrap your router in a `TestClient` and call HTTP methods:

```rust
use ravel_test::TestClient;

let client = TestClient::new(router);
```

### Request Methods

| Method                  | Description                              |
|-------------------------|------------------------------------------|
| `get(uri)`              | Send a GET request                       |
| `post_json(uri, body)`  | Send a POST with JSON body               |
| `put_json(uri, body)`   | Send a PUT with JSON body                |
| `patch_json(uri, body)` | Send a PATCH with JSON body              |
| `delete(uri)`           | Send a DELETE request                    |
| `patch(uri)`            | Send a PATCH request (no body)           |

All methods are `async` and return a `TestResponse`.

```rust
let resp = client.get("/users").await;
let resp = client.post_json("/users", r#"{"name":"Alice"}"#).await;
let resp = client.put_json("/users/1", r#"{"name":"Bob"}"#).await;
let resp = client.delete("/users/1").await;
```

## TestResponse Assertions

`TestResponse` wraps the Axum response and provides fluent assertion methods.

### Status Assertions

```rust
resp.assert_ok();              // 200 OK
resp.assert_created();         // 201 Created
resp.assert_redirect();        // 302 Found
resp.assert_unauthorized();    // 401 Unauthorized
resp.assert_forbidden();       // 403 Forbidden
resp.assert_not_found();       // 404 Not Found
resp.assert_unprocessable();   // 422 Unprocessable Entity
resp.assert_status(StatusCode::OK);  // any custom status
```

### Body Assertions

```rust
// Check the body contains a string
resp.assert_see("Welcome, Alice");

// Check the body does NOT contain a string
resp.assert_dont_see("Error");

// Parse the body as JSON and compare
resp.assert_json(serde_json::json!({"status": "ok"}));

// Get the body text directly
let text = resp.text().to_string();

// Parse as a typed JSON value
let data: MyStruct = resp.json().unwrap();
```

## No Running Server Needed

`TestClient` uses `tower::ServiceExt::oneshot` to dispatch requests directly
into the router. There is no TCP or HTTP overhead, no port conflicts, and no
server lifecycle to manage.

```rust
use ravel_test::TestClient;
use axum::{Router, routing::get, Json};
use serde::Serialize;

async fn list_users() -> Json<Vec<String>> {
    Json(vec!["Alice".into(), "Bob".into()])
}

fn test_app() -> Router {
    Router::new()
        .route("/users", get(list_users))
}

#[tokio::test]
async fn test_list_users() {
    let client = TestClient::new(test_app());
    let resp = client.get("/users").await;
    resp.assert_ok();
    resp.assert_see("Alice");
}
```

## Example: Full CRUD Test Suite

```rust
use ravel_test::TestClient;
use axum::{Router, routing::{get, post, delete}, Json};
use serde::{Serialize, Deserialize};
use std::sync::{Mutex, Arc};

#[derive(Serialize, Deserialize, Clone)]
struct User {
    id: u32,
    name: String,
}

type Db = Arc<Mutex<Vec<User>>>;

fn app() -> Router {
    let db = Db::default();
    Router::new()
        .route("/users", get({
            let db = db.clone();
            move || {
                let users = db.lock().unwrap();
                Json(users.clone())
            }
        }))
        .route("/users", post({
            let db = db.clone();
            move || async {
                db.lock().unwrap().push(User { id: 1, name: "Alice".into() });
                (axum::http::StatusCode::CREATED, Json("created"))
            }
        }))
}

#[tokio::test]
async fn test_create_user() {
    let client = TestClient::new(app());
    let resp = client.post_json("/users", r#"{"name":"Alice"}"#).await;
    resp.assert_created();
}

#[tokio::test]
async fn test_list_users_contains_created() {
    let client = TestClient::new(app());
    let resp = client.get("/users").await;
    resp.assert_ok();
    resp.assert_see("Alice");
}

#[tokio::test]
async fn test_returns_404() {
    let client = TestClient::new(app());
    let resp = client.get("/nonexistent").await;
    resp.assert_not_found();
}
```
