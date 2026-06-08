# Authentication

Ravel provides a session-based authentication system through the `Auth` facade and the `AuthGuard` middleware. Authentication is tied to the HTTP request lifecycle — the facade is only available inside request handlers.

## The Auth Facade

The `Auth` facade uses `tokio::task_local!` storage, set automatically by a framework middleware. Outside an HTTP request scope, all methods return `false` or `None`.

```rust
use ravel_facades::Auth;

// Check authentication status
if Auth::check() {
    // User is logged in
}

if Auth::guest() {
    // Not logged in
}

// Get the authenticated user's ID
let user_id: Option<u32> = Auth::id();

// Login / logout
Auth::login(&42u32);
Auth::logout();
```

## Login Handler

A typical login handler reads credentials, validates them, and calls `Auth::login`:

```rust
use ravel_facades::{Auth, Session, redirect, response};
use ravel_http::error::RavelError;
use axum::response::IntoResponse;

async fn login_handler(
    axum::Form(form): axum::Form<LoginForm>,
) -> Result<impl IntoResponse, RavelError> {
    let user = find_user_by_email(&form.email).await?;

    if !Hash::check(&form.password, &user.password_hash)? {
        return Err(RavelError::unauthorized("Invalid credentials"));
    }

    Auth::login(&user.id);

    Session::flash("success", &"Welcome back!");
    Ok(redirect("/dashboard"))
}
```

## Logout Handler

```rust
async fn logout_handler() -> impl IntoResponse {
    Auth::logout();
    redirect("/login")
}
```

## Using the Low-Level Extractor

You can also work with the raw `Session` extractor directly. The `Auth` struct in `ravel_http` requires a `&Session`:

```rust
use ravel_http::auth::Auth;
use ravel_http::session::Session;

async fn dashboard(mut session: Session) -> impl IntoResponse {
    if Auth::guest(&session) {
        return redirect("/login");
    }
    let user_id: u32 = Auth::id(&session).unwrap();
    format!("Welcome, user {}!", user_id)
}
```

## Protecting Routes with AuthGuard

The `AuthGuard` middleware checks for a session cookie and returns 401 if the user is not authenticated:

```rust
use ravel_facades::Route;
use ravel_http::auth::AuthGuard;

// Protect a group of admin routes
Route::group("/admin", || {
    Route::get("/dashboard", admin_dashboard);
    Route::get("/users", admin_users);
});
Route::middleware(AuthGuard::middleware());

let router = Route::build();
```

Only routes registered after `Route::middleware()` are affected. GET requests that lack a valid session receive a 401 JSON response.

## How It Works

1. The session middleware reads the `ravel_session` cookie, decrypts it using AES-256-GCM, and populates the request context.
2. `Auth::login` stores the user ID in the request context under `_auth_id`.
3. On subsequent requests, the persisted session cookie is decrypted and the auth ID is restored.
4. `AuthGuard` rejects requests that have no session cookie at all.

## Outside HTTP Scope

When called from a CLI command or a queue job, `Auth` methods return safe defaults:

```rust
// In a CLI command
assert_eq!(Auth::check(), false);  // always false
assert_eq!(Auth::id::<u32>(), None); // always None
```
