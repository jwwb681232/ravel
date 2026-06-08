# CSRF Protection

Cross-Site Request Forgery (CSRF) protection is provided by the `Csrf` struct. It uses **HMAC-based token generation and verification** with constant-time comparison to prevent timing attacks.

## How It Works

1. A CSRF token is generated as a random payload combined with an HMAC-SHA256 signature.
2. Tokens are verified by checking the HMAC signature against the application key.
3. Only state-changing methods (`POST`, `PUT`, `PATCH`, `DELETE`) are checked.
4. Safe methods (`GET`, `HEAD`, `OPTIONS`) pass through without verification.

## Setup

Create a `Csrf` instance with your application key and add its middleware:

```rust
use ravel_http::csrf::Csrf;

let csrf = Csrf::new("base64-encoded-app-key");

Route::group("/api", || {
    Route::post("/posts", create_post);
    Route::put("/posts/{id}", update_post);
    Route::delete("/posts/{id}", delete_post);
});
Route::middleware(csrf.middleware());

let router = Route::build();
```

## Generating Tokens

In your templates, generate a CSRF token and include it in forms or AJAX requests:

```rust
use ravel_http::csrf::Csrf;

// Generate a new token (usually in a view context)
let csrf = Csrf::new(app_key);
let token = csrf.generate();

// Pass to template
// {{ csrf_field }} => <input type="hidden" name="_csrf" value="..." />
```

## Sending the Token

For HTML forms, include the token in a hidden field or in the `X-CSRF-TOKEN` header:

```html
<form method="POST" action="/posts">
    <input type="hidden" name="_csrf" value="{{ csrf_token }}">
    <!-- form fields -->
</form>
```

For AJAX requests, set the header:

```javascript
// Using fetch
fetch('/posts', {
    method: 'POST',
    headers: {
        'X-CSRF-TOKEN': csrfToken,
        'Content-Type': 'application/json',
    },
    body: JSON.stringify(data),
});
```

The middleware reads the token from either:
- `X-CSRF-TOKEN` header (primary)
- `X-XSRF-TOKEN` header (fallback, common in SPA frameworks)

## On Token Mismatch

When a state-changing request is received without a valid CSRF token, the middleware returns a **419** status code:

```json
{
    "message": "CSRF token mismatch",
    "status": 419
}
```

## Full Example

```rust
use ravel_http::csrf::Csrf;
use ravel_facades::Route;

fn configure_routes(app_key: &[u8]) {
    let csrf = Csrf::new(app_key);

    Route::get("/posts/new", || async {
        // Render a form with a CSRF token
        let token = csrf.generate();
        format!(
            r#"<form method="POST" action="/posts">
                  <input type="hidden" name="_csrf" value="{}">
                  <input name="title"><button>Create</button>
               </form>"#,
            token
        )
    });

    Route::post("/posts", || async { "Post created!" });

    // Apply CSRF protection to state-changing routes
    Route::middleware(csrf.middleware());
}
```

## API Reference

| Method | Description |
|--------|-------------|
| `Csrf::new(key)` | Create a CSRF protector with the given key |
| `generate()` | Produce a new `{payload}.{hmac}` token |
| `verify(token)` | Constant-time token verification |
| `middleware()` | Returns an Axum middleware layer |
