# Service Container

The service container is Ravel's dependency injection engine. It manages object creation, resolution, and lifecycle. All facades ultimately delegate to services stored in the container.

## Binding Services

Services are bound to the container by **type** (via Rust's `TypeId`). There's no string-key lookup — everything is compile-time checked.

### Singletons

A singleton is an object that is created once and shared across the entire application:

```rust
use ravel_core::container::Container;
use std::sync::Arc;

struct Database { url: String }

let container = Container::new();
container.singleton(|_| Database { url: "postgres://...".into() });

// Every resolve() returns the same Arc<Database>
let db1: Arc<Database> = container.resolve().unwrap();
let db2: Arc<Database> = container.resolve().unwrap();
// Arc::as_ptr(&db1) == Arc::as_ptr(&db2) — same instance
```

### Pre-built Instances

```rust
container.instance(42u32);
assert_eq!(*container.resolve::<u32>().unwrap(), 42);
```

### Transient Factories

Each `resolve_fresh()` call creates a new instance:

```rust
container.bind(|_: &Container| rand::random::<u32>());
let a = container.resolve_fresh::<u32>().unwrap();
let b = container.resolve_fresh::<u32>().unwrap();
// a != b — different values
```

### Trait Objects (Key-Type Idiom)

```rust
trait Logger: Send + Sync { fn log(&self, msg: &str); }
struct StdoutLogger;
impl Logger for StdoutLogger { fn log(&self, msg: &str) { println!("{msg}"); } }

struct LoggerService; // key type

container.bind_trait::<LoggerService, dyn Logger>(Arc::new(StdoutLogger));
let logger = container.resolve_trait::<LoggerService, dyn Logger>().unwrap();
```

## Resolution

### From Handlers

In Axum handlers, the container is available via the `APP` global:

```rust
use ravel_core::app::APP;

async fn handler() -> impl IntoResponse {
    let db = APP.get().unwrap().container().resolve::<Database>().unwrap();
    // ...
}
```

### In Service Providers

During `register()` and `boot()`, the provider receives a `&Container` reference:

```rust
use ravel_core::app::ServiceProvider;
use ravel_core::container::Container;
use anyhow::Result;

struct DatabaseProvider;

impl ServiceProvider for DatabaseProvider {
    fn register(&self, container: &Container) -> Result<()> {
        container.singleton(|_| Database::connect());
        Ok(())
    }
    fn name(&self) -> &str { "DatabaseProvider" }
}
```

## Freeze

After `Application::boot()`, the container calls `freeze()`, which locks it for read-only access. All write methods (`singleton`, `instance`, `bind`) will **panic** if called after freeze.

This makes the container safe for concurrent resolution across all Axum worker threads with zero lock contention.

## Convenience Methods

The `Application` builder provides shorthand methods for common registrations:

```rust
use ravel_core::app::Application;

Application::new()
    .with_cache()                          // register MemoryCache
    .with_app_key("base64:...")?           // register Crypt
    .register_provider(MyProvider)
    .boot()?;
```
