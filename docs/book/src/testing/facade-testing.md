# Facade Testing

Ravel's facades (Config, Cache, Hash, Route, etc.) read from global state
behind the scenes. Writing tests that use facades requires a few setup steps to
ensure isolation between test cases.

## Resetting Global State Between Tests

Two globals need to be reset before each test that uses facades:

```rust
use ravel_core::app::APP;
use ravel_facades::Route;

// Before each test:
APP.reset();
Route::reset();
```

- **`APP.reset()`** — clears the global `Application` singleton so the next
  call to `Application::boot()` succeeds.
- **`Route.reset()`** — clears all previously registered routes from the
  static route registry.

Call both before booting a fresh application:

```rust
#[test]
fn test_something() {
    APP.reset();
    Route::reset();

    Application::new()
        .register_provider(MyProvider)
        .boot()
        .unwrap();

    // ... assertions using facades ...
}
```

## The TEST_LOCK Pattern

Multiple tests that boot the application cannot run concurrently because they
would race on the global `APP` singleton. Use a static `Mutex<()>` to
serialise them:

```rust
use std::sync::Mutex;

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
```

Acquire the lock at the top of every test that boots the application:

```rust
#[test]
fn test_config_get() {
    let _l = lock();      // serialises with other boot tests
    APP.reset();
    Route::reset();

    Application::new()
        .register_provider(TestProvider)
        .boot()
        .unwrap();

    let val: Option<String> = Config::get("app.name");
    assert_eq!(val, None);
}
```

The `_l` binding holds the guard for the duration of the test. When the test
completes (or panics), the guard is dropped and the next test can proceed.

## Testing Facade Errors with should_panic

Some facades panic when their backing service is not registered. Use
`#[should_panic]` to verify this behaviour:

```rust
#[test]
#[should_panic(expected = "MemoryCache not registered")]
fn test_cache_panics_without_with_cache() {
    let _l = lock();
    APP.reset();
    Application::new().boot().unwrap();

    Cache::put("key", "val".to_string(), None);
    // ^ panics because Application::with_cache() was never called
}
```

```rust
#[test]
#[should_panic(expected = "Application not booted")]
fn test_facades_panic_before_boot() {
    let _l = lock();
    APP.reset();

    Config::get::<String>("any.key");
    // ^ panics because Application::boot() was never called
}
```

The `TEST_LOCK` and `APP.reset()` are important here — `#[should_panic]` tests
may leave the `APP` mutex in a poisoned state. `APP.reset()` recovers from
poisoned mutexes so subsequent tests can continue.

## Example: Testing Config and Cache Facades

```rust
use std::sync::Mutex;
use std::time::Duration;
use ravel_core::app::{Application, APP};
use ravel_facades::{Config, Cache, Route};

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_config_get_or() {
    let _l = TEST_LOCK.lock().unwrap();
    APP.reset();
    Route::reset();

    Application::new()
        .register_provider(TestProvider)
        .boot()
        .unwrap();

    let port: u16 = Config::get_or("server.port", 8080);
    assert_eq!(port, 8080);
}

#[test]
fn test_cache_put_and_get() {
    let _l = TEST_LOCK.lock().unwrap();
    APP.reset();
    Route::reset();

    Application::new()
        .with_cache()
        .register_provider(TestProvider)
        .boot()
        .unwrap();

    Cache::put("my_key", 42u32, Some(Duration::from_secs(60)));
    let val: Option<u32> = Cache::get("my_key");
    assert_eq!(val, Some(42));
    assert!(Cache::has("my_key"));

    Cache::forget("my_key");
    assert!(!Cache::has("my_key"));
}
```

## Example: Testing Route Registration

```rust
use ravel_core::app::{Application, ServiceProvider, APP};
use ravel_core::container::Container;
use ravel_facades::Route;
use ravel_test::TestClient;
use anyhow::Result;
use std::sync::Mutex;

static TEST_LOCK: Mutex<()> = Mutex::new(());

struct RouteProvider;

impl ServiceProvider for RouteProvider {
    fn register(&self, _c: &Container) -> Result<()> {
        Route::get("/hello", || async { "Hello, Ravel!" });
        Ok(())
    }
    fn name(&self) -> &str { "RouteProvider" }
}

#[tokio::test]
async fn test_route_returns_hello() {
    let _l = TEST_LOCK.lock().unwrap();
    APP.reset();
    Route::reset();

    Application::new()
        .register_provider(RouteProvider)
        .boot()
        .unwrap();

    let router = Route::build();
    let client = TestClient::new(router);
    let resp = client.get("/hello").await;
    resp.assert_ok();
    resp.assert_see("Hello, Ravel!");
}
```

By resetting both `APP` and `Route` before each test, and serialising with
`TEST_LOCK`, multiple facade-dependent tests can coexist in the same test
suite without interfering with each other.
