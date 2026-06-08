use anyhow::Result;
use ravel_core::app::Application;
use ravel_core::app::ServiceProvider;
use ravel_core::container::Container;
use ravel_facades::response;
use ravel_facades::{Cache, Config, Hash, Route, env, env_or, now, redirect};
use std::sync::Mutex;
use std::time::Duration;

/// Serialize tests that share the global APP singleton.
static TEST_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// Helper: provider that registers routes via the Route facade
struct TestProvider;

impl ServiceProvider for TestProvider {
    fn register(&self, _c: &Container) -> Result<()> {
        Route::get("/hello", || async { "Hello from facade!" });
        Ok(())
    }
    fn name(&self) -> &str {
        "TestProvider"
    }
}

// ── Config facade tests ──────────────────────────────────────────────

#[test]
fn test_config_get_after_boot() {
    let _l = lock();
    Route::reset();
    ravel_core::app::APP.reset();
    Application::new()
        .register_provider(TestProvider)
        .boot()
        .unwrap();

    let val: Option<String> = Config::get("nonexistent.key");
    assert_eq!(val, None);
}

#[test]
fn test_config_get_or_with_default() {
    let _l = lock();
    Route::reset();
    ravel_core::app::APP.reset();
    Application::new()
        .register_provider(TestProvider)
        .boot()
        .unwrap();

    let val = Config::get_or("nonexistent", 42u16);
    assert_eq!(val, 42);
}

// ── Cache facade tests ───────────────────────────────────────────────

#[test]
fn test_cache_put_and_get() {
    let _l = lock();
    Route::reset();
    ravel_core::app::APP.reset();
    Application::new()
        .with_cache()
        .register_provider(TestProvider)
        .boot()
        .unwrap();

    Cache::put(
        "test_key",
        "test_value".to_string(),
        Some(Duration::from_secs(60)),
    );
    let val: Option<String> = Cache::get("test_key");
    assert_eq!(val, Some("test_value".to_string()));
}

#[test]
fn test_cache_miss() {
    let _l = lock();
    Route::reset();
    ravel_core::app::APP.reset();
    Application::new()
        .with_cache()
        .register_provider(TestProvider)
        .boot()
        .unwrap();

    let val: Option<String> = Cache::get("nonexistent_key");
    assert_eq!(val, None);
}

#[test]
fn test_cache_has_and_forget() {
    let _l = lock();
    Route::reset();
    ravel_core::app::APP.reset();
    Application::new()
        .with_cache()
        .register_provider(TestProvider)
        .boot()
        .unwrap();

    assert!(!Cache::has("forget_me"));
    Cache::put("forget_me", 42u32, None);
    assert!(Cache::has("forget_me"));
    Cache::forget("forget_me");
    assert!(!Cache::has("forget_me"));
}

// ── Hash facade tests ────────────────────────────────────────────────

#[test]
fn test_hash_make_and_check() {
    let _l = lock();
    ravel_core::app::APP.reset();
    Application::new().boot().unwrap();

    let hashed = Hash::make("secret123").unwrap();
    assert!(Hash::check("secret123", &hashed).unwrap());
    assert!(!Hash::check("wrong_password", &hashed).unwrap());
}

// ── Route facade test ────────────────────────────────────────────────

#[test]
fn test_route_facade_builds_router() {
    let _l = lock();
    ravel_core::app::APP.reset();
    Route::reset();

    Application::new()
        .register_provider(TestProvider)
        .boot()
        .unwrap();

    let router = Route::build();
    assert!(!format!("{:?}", router).is_empty());
}

// ── Utility facade tests ─────────────────────────────────────────────

#[test]
fn test_now_returns_current_time() {
    let t1 = now();
    std::thread::sleep(Duration::from_millis(10));
    let t2 = now();
    assert!(t2 > t1);
}

#[test]
fn test_env_returns_value_or_none() {
    unsafe { std::env::set_var("RAVEL_FACADE_TEST_VAR", "hello") };
    assert_eq!(env("RAVEL_FACADE_TEST_VAR"), Some("hello".to_string()));
    assert_eq!(env("RAVEL_NONEXISTENT_VAR_XYZ"), None);
    assert_eq!(env_or("RAVEL_NONEXISTENT_VAR_XYZ", "fallback"), "fallback");
    unsafe { std::env::remove_var("RAVEL_FACADE_TEST_VAR") };
}

// ── Response helper tests ────────────────────────────────────────────

#[test]
fn test_redirect_returns_302() {
    use axum::response::IntoResponse;
    let resp = redirect("/login").into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::FOUND);
    assert!(
        resp.headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("/login")
    );
}

#[test]
fn test_response_builder_creates_response() {
    use axum::response::IntoResponse;
    let resp = response::response()
        .status(axum::http::StatusCode::CREATED)
        .body("test body")
        .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::CREATED);
}

// ── Collection tests ─────────────────────────────────────────────────

#[test]
fn test_collection_map_filter_sort() {
    let result = ravel_facades::collect!(vec!["carol", "alice", "bob"])
        .map(|s| s.to_uppercase())
        .filter(|s| !s.is_empty())
        .sort()
        .to_vec();

    assert_eq!(result, vec!["ALICE", "BOB", "CAROL"]);
}

#[test]
fn test_collection_reject() {
    let result = ravel_facades::collect!(vec![1, 2, 3, 4, 5])
        .reject(|&x| x % 2 == 0)
        .to_vec();

    assert_eq!(result, vec![1, 3, 5]);
}

#[test]
fn test_collection_implode() {
    let result = ravel_facades::collect!(vec!["a", "b", "c"]).implode(", ");
    assert_eq!(result, "a, b, c");
}

// ── Panic tests ──────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "MemoryCache not registered")]
fn test_cache_panics_without_with_cache() {
    let _l = lock();
    ravel_core::app::APP.reset();
    Application::new().boot().unwrap();
    Cache::put("key", "val".to_string(), None);
}

#[test]
#[should_panic(expected = "Application not booted")]
fn test_facades_panic_before_boot() {
    let _l = lock();
    ravel_core::app::APP.reset();
    Config::get::<String>("any.key");
}
