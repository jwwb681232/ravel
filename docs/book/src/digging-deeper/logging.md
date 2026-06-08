# Logging

Ravel provides structured logging through the `Log` facade, which re-exports the macros
from the `tracing` crate. In debug builds output is human-readable; in release builds it
produces JSON for machine parsing.

## Initialisation

Call `Log::init()` during application bootstrap to set the default log level. The
`RAVEL_LOG` environment variable overrides the default level when present.

```rust
use ravel_facades::Log;

Log::init("info");
// RAVEL_LOG=debug would override to debug level
```

## Logging Macros

The `Log` facade re-exports `info!`, `error!`, `warn!`, `debug!`, and `trace!`. These
behave identically to the `tracing` versions.

```rust
use ravel_facades::Log;

Log::info!("Application started");
Log::error!("Failed to connect: {}", err);
Log::warn!("Rate limit approaching for IP {}", ip);
Log::debug!("Query result: {:?}", rows);
Log::trace!("Entering loop iteration {}", i);
```

## Structured Fields

All macros support structured key-value fields alongside the human-readable message.

```rust
Log::info!(
    user_id = 42,
    action = "login",
    ip = "192.168.1.1",
    "User logged in"
);

Log::error!(
    error = %err,
    route = "/api/users",
    method = "POST",
    "Request failed"
);
```

## Log Level via RAVEL_LOG

Set `RAVEL_LOG` to control the active log level at runtime without recompiling.

```bash
export RAVEL_LOG=debug   # Trace everything
export RAVEL_LOG=info    # Info, warnings, and errors (default)
export RAVEL_LOG=error   # Only errors
```

## Output Formats

| Build Profile | Format | Use Case |
|---------------|--------|----------|
| Debug (`cargo run`) | Human-readable, colourised | Local development |
| Release (`cargo build --release`) | JSON lines | Production log aggregation |

In release mode each log line is a JSON object:

```json
{"timestamp":"2026-06-08T12:00:00Z","level":"INFO","fields":{"message":"Server started"}}
```

## Example: Structured Logging in Handlers

```rust
use ravel_facades::{Log, Route};
use ravel_http::error::RavelError;

async fn create_user(
    Json(payload): Json<CreateUser>,
) -> Result<impl IntoResponse, RavelError> {
    Log::info!(
        email = %payload.email,
        name = %payload.name,
        "Creating new user"
    );

    match User::create(payload).await {
        Ok(user) => {
            Log::info!(user_id = user.id, "User created successfully");
            Ok(response().json(user)?)
        }
        Err(e) => {
            Log::error!(
                error = %e,
                email = %payload.email,
                "Failed to create user"
            );
            Err(RavelError::Internal(e.into()))
        }
    }
}
```

## API Reference

| Macro | Purpose |
|-------|---------|
| `Log::info!(...)` | Informational messages |
| `Log::error!(...)` | Runtime errors and failures |
| `Log::warn!(...)` | Warning conditions |
| `Log::debug!(...)` | Debug-level details |
| `Log::trace!(...)` | Fine-grained tracing |

## Note

All `Log::*` macros are re-exports of the corresponding `tracing` macros. You can also use
`tracing::info!` directly in your application code — the subscriber is global and affects
all crates.
