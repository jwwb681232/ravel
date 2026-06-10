# Sessions

Ravel supports **two session drivers**:

1. **Cookie** (default) — session data is serialised as JSON, encrypted with AES-256-GCM, and stored in the client cookie. No server-side storage needed.
2. **Redis** — session ID stored in cookie, data stored in Redis. Enables horizontal scaling across multiple workers.

## Cookie Driver

No database required. Data is encrypted in the cookie itself.

```rust
use ravel_http::session::SessionConfig;
use ravel_core::crypt::Crypt;

let crypt = Crypt::from_app_key()?;
let config = SessionConfig::new(crypt)
    .cookie_name("ravel_session")
    .max_age(7200); // 2 hours

Route::new()
    .layer(config.layer())
    .build();
```

## Redis Driver

Enable the `redis` feature in `Cargo.toml`:

```toml
ravel-http = { features = ["redis"] }
```

```rust
let config = SessionConfig::redis("redis://127.0.0.1:6379", crypt).await?;
let config = config
    .cookie_name("ravel_session")
    .max_age(7200);

Route::new()
    .layer(config.layer())
    .build();
```

How it works:

- Cookie contains only a **UUID session ID**
- Data is stored at `ravel:session:{id}` with TTL = `max_age`
- Multiple workers share sessions via atomic Redis `SET`/`GET`

## Reading and Writing Data

```rust
use ravel_http::session::Session;

async fn handler(mut session: Session) -> impl IntoResponse {
    // Store
    session.put("user_id", 42u32);
    session.put("theme", "dark");

    // Retrieve
    let user_id: Option<u32> = session.get("user_id");

    // Check
    if session.has("theme") { ... }

    // Remove
    session.forget("theme");

    // All keys
    for key in session.keys() {
        println!("{key}");
    }

    "ok"
}
```

## Flash Messages

Flash messages persist for exactly **one** subsequent request, then disappear. Useful for success/error feedback after redirects:

```rust
// Store a flash message
session.flash("status", "Profile updated!");

// Next request — read (consumes the message)
let status: Option<String> = session.flashed("status");
// → Some("Profile updated!")

// Second read returns None
let again: Option<String> = session.flashed("status");
// → None
```

## Session Configuration Reference

| Method | Default | Description |
|--------|---------|-------------|
| `cookie_name(name)` | `"ravel_session"` | Cookie name |
| `max_age(secs)` | `7200` | Session TTL in seconds |

### Cookie Attributes
All sessions use: `HttpOnly; SameSite=Lax; Path=/; Max-Age={ttl}`

### Redis key format
```
ravel:session:{session_id}  →  JSON-encoded SessionData
```
