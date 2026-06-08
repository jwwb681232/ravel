# Sessions

Ravel uses **encrypted cookie sessions** — session data is serialised as JSON, encrypted with AES-256-GCM, and stored client-side. This requires no server-side storage and works out of the box once you configure an application key.

## Configuration

Sessions are configured via `SessionConfig` and wired in as a Tower layer:

```rust
use ravel_http::session::SessionConfig;
use ravel_core::crypt::Crypt;

let crypt = Crypt::from_app_key("base64:...")?;
let session_config = SessionConfig::new(crypt)
    .cookie_name("myapp_session")   // default: "ravel_session"
    .max_age(3600);                  // default: 7200 (2 hours)

// Apply to routes:
Route::middleware(session_config.layer());
```

The default session lifetime is **2 hours** (7200 seconds). The cookie is marked `HttpOnly`, `SameSite=Lax`, and `Secure` for non-localhost connections.

## The Session Facade

Inside an HTTP handler, use the `Session` facade for read and write operations:

```rust
use ravel_facades::Session;

// Store a value
Session::put("user_preferences", &serde_json::json!({
    "theme": "dark",
    "locale": "en"
}));

// Read a value
let theme: Option<String> = Session::get("theme");
let locale: Option<String> = Session::get("locale");

// Check if a key exists
if Session::has("user_preferences") {
    // preferences exist
}

// Remove a key
Session::forget("user_preferences");
```

## Using the Session Extractor

For more control, use the Axum `Session` extractor. This gives you mutable access:

```rust
use ravel_http::session::Session;

async fn save_preferences(
    mut session: Session,
    axum::Form(form): axum::Form<Preferences>,
) -> impl IntoResponse {
    session.put("theme", &form.theme);
    session.put("locale", &form.locale);
    "Preferences saved"
}

async fn get_preferences(session: Session) -> impl IntoResponse {
    let theme: Option<String> = session.get("theme");
    let locale: Option<String> = session.get("locale");
    format!("theme={:?}, locale={:?}", theme, locale)
}
```

## Flash Messages

Flash messages are stored for the **next request only** and are consumed on first read. They are ideal for success/error messages after a form submit or redirect:

```rust
use ravel_facades::{Session, redirect};

async fn submit_form() -> impl IntoResponse {
    // Process form ...
    Session::flash("success", &"Form submitted successfully!");
    redirect("/dashboard")
}

async fn dashboard() -> impl IntoResponse {
    let message: Option<String> = Session::flashed("success");
    match message {
        Some(msg) => format!("<div class='alert'>{}</div>", msg),
        None => "Dashboard".into(),
    }
}
```

The `flashed` method consumes the value on first read — subsequent calls return `None`. The low-level extractor works the same way:

```rust
let msg: Option<String> = session.flashed("success");
```

## Example: Storing User Preferences

```rust
async fn save_theme(mut session: Session, axum::Query(params): axum::Query<HashMap<String, String>>) -> impl IntoResponse {
    if let Some(theme) = params.get("theme") {
        session.put("theme", theme);
    }
    redirect("/settings")
}

async fn settings(session: Session) -> impl IntoResponse {
    let theme: Option<String> = session.get("theme");
    let theme_css = theme.unwrap_or_else(|| "light".into());
    format!("<link rel='stylesheet' href='/css/{}.css'>", theme_css)
}
```

## Outside HTTP Scope

Like `Auth`, the `Session` facade returns safe defaults outside request handlers:

```rust
assert_eq!(Session::get::<String>("key"), None);
Session::put("key", &"value"); // no-op
```
