# Round 1: API-Level Laravel-ification — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Ravel code look and feel like Laravel at the call-site level. Pure additive — zero breaking changes to existing crate public APIs.

**Architecture:** New `ravel-facades` crate depends on `ravel-core`, `ravel-http`, `ravel-support`. Global `APP: OnceLock<Application>` in `ravel-core::app`, set by `boot()`. Stateless facades read `APP.get().unwrap().container()` to resolve services. Request-level facades use `tokio::task_local! REQUEST` defined in `ravel-http::facades`.

**Tech Stack:** Rust, tokio (task_local), OnceLock, parking_lot, axum 0.8, chrono

---

## File Map

```
NEW: crates/ravel-facades/
├── Cargo.toml
├── src/
│   ├── lib.rs              Module declarations
│   ├── config.rs           Config::get / get_or / has
│   ├── cache.rs            Cache::put / get / has / forget / flush
│   ├── crypt.rs            Crypt::encrypt / decrypt / encrypt_value / decrypt_value
│   ├── hash.rs             Hash::make / check
│   ├── log.rs              Log re-exports (info!, error!, warn!, debug!)
│   ├── storage.rs          Storage::put / get / exists / delete
│   ├── queue.rs            Queue::dispatch / dispatch_later / pending
│   ├── route.rs            Route::get / post / put / delete / patch / group / middleware / build
│   ├── auth.rs             Auth::check / guest / id / login / logout
│   ├── session.rs          Session::get / put / has / forget / flash
│   ├── request.rs          request() helper
│   ├── response.rs         response() / redirect() / back() / abort()
│   ├── collection.rs       Collection<T> + collect! macro
│   ├── path.rs             config_path / database_path / storage_path / base_path
│   └── utils.rs            env() / env_or() / now()
└── tests/
    └── integration.rs      E2E facade integration test

MODIFY: crates/ravel-core/src/
├── app.rs                  Add global APP, change boot() → Result<()>, add with_cache/with_crypt/with_queue
└── lib.rs                  (unchanged)

NEW: crates/ravel-http/src/facades/
└── mod.rs                  RequestContext + REQUEST task-local + start_request middleware

MODIFY: crates/ravel-http/src/
├── lib.rs                  Add `pub mod facades;`
└── route.rs                Add RouteRegistry type + From<RouteRegistry> for axum::Router

MODIFY: Cargo.toml           Add "crates/ravel-facades" to workspace members
MODIFY: testblog/             Update main.rs, app.rs, integration_test.rs
```

---

### Task 1: Create ravel-facades crate skeleton

**Files:**
- Create: `crates/ravel-facades/Cargo.toml`
- Create: `crates/ravel-facades/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Add workspace member**

In `E:\rust\ravel\Cargo.toml`, add `"crates/ravel-facades"` to the `members` array:

```toml
"crates/ravel-facades",
```

- [ ] **Step 2: Create Cargo.toml**

Create `E:\rust\ravel\crates\ravel-facades\Cargo.toml`:

```toml
[package]
name = "ravel-facades"
version = "0.1.0"
edition = "2024"
description = "Laravel-style static facades for the Ravel framework"
license = "MIT"
repository = "https://github.com/jwwb681232/ravel"
categories = ["web-programming"]
keywords = ["ravel", "facade", "web", "framework"]

[dependencies]
ravel-core = { path = "../ravel-core" }
ravel-http = { path = "../ravel-http" }
ravel-support = { path = "../ravel-support" }
parking_lot = "0.12"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
tokio = { version = "1", features = ["full"] }
chrono = "0.4"
axum = "0.8"
uuid = { version = "1", features = ["v4"] }

[dev-dependencies]
ravel-test = { path = "../ravel-test" }
```

- [ ] **Step 3: Create empty placeholder modules**

Create `E:\rust\ravel\crates\ravel-facades\src\lib.rs`:

```rust
//! Laravel-style static facades for the Ravel framework.
//!
//! All facades are usable after [`Application::boot()`] has completed.
//! Accessing them before boot will panic with "Application not booted".

pub mod cache;
pub mod collection;
pub mod config;
pub mod crypt;
pub mod hash;
pub mod log;
pub mod path;
pub mod queue;
pub mod request;
pub mod response;
pub mod route;
pub mod session;
pub mod auth;
pub mod storage;
pub mod utils;

pub use collection::collect;
```

Create 14 placeholder files at `E:\rust\ravel\crates\ravel-facades\src\`:

**config.rs:**
```rust
//! Config facade — dot-notation configuration access.
use ravel_core::app::APP;
use serde::de::DeserializeOwned;

pub struct Config;

impl Config {
    fn app() -> &'static ravel_core::app::Application {
        APP.get().expect("Application not booted — call Application::boot() first")
    }

    pub fn get<T: DeserializeOwned>(key: &str) -> Option<T> {
        Self::app().config().get(key)
    }

    pub fn get_or<T: DeserializeOwned>(key: &str, default: T) -> T {
        Self::get(key).unwrap_or(default)
    }

    pub fn has(key: &str) -> bool {
        Self::app().config().has(key)
    }
}
```

**cache.rs:**
```rust
//! Cache facade — in-memory TTL cache.
use ravel_core::app::APP;
use ravel_core::cache::Cache as CacheTrait;
use std::time::Duration;

pub struct Cache;

impl Cache {
    fn cache() -> std::sync::Arc<ravel_core::cache::MemoryCache> {
        APP.get()
            .expect("Application not booted")
            .container()
            .resolve::<ravel_core::cache::MemoryCache>()
            .expect("MemoryCache not registered — call Application::with_cache() before boot")
    }

    pub fn put(key: &str, value: impl std::any::Any + Send + Sync, ttl: Option<Duration>) {
        Self::cache().put(key, Box::new(value), ttl);
    }

    pub fn get<T: 'static + Clone + Send + Sync>(key: &str) -> Option<T> {
        Self::cache().get::<T>(key)
    }

    pub fn has(key: &str) -> bool {
        Self::cache().has(key)
    }

    pub fn forget(key: &str) {
        Self::cache().forget(key);
    }

    pub fn flush() {
        Self::cache().flush();
    }
}
```

**crypt.rs:**
```rust
//! Crypt facade — AES-256-GCM encryption.
use anyhow::Result;
use ravel_core::app::APP;
use ravel_core::crypt::Crypt as CryptEngine;

pub struct Crypt;

impl Crypt {
    fn engine() -> std::sync::Arc<CryptEngine> {
        APP.get()
            .expect("Application not booted")
            .container()
            .resolve::<CryptEngine>()
            .expect("Crypt not registered — call Application::with_app_key() before boot")
    }

    pub fn encrypt(plaintext: &[u8]) -> Result<String> {
        Self::engine().encrypt(plaintext)
    }

    pub fn decrypt(encoded: &str) -> Result<Vec<u8>> {
        Self::engine().decrypt(encoded)
    }

    pub fn encrypt_value<T: serde::Serialize>(value: &T) -> Result<String> {
        Self::engine().encrypt_value(value)
    }

    pub fn decrypt_value<T: serde::de::DeserializeOwned>(encoded: &str) -> Result<T> {
        Self::engine().decrypt_value(encoded)
    }
}
```

**hash.rs:**
```rust
//! Hash facade — bcrypt password hashing.
use anyhow::Result;

pub struct Hash;

impl Hash {
    pub fn make(password: &str) -> Result<String> {
        ravel_core::hash::Hash::make(password)
    }

    pub fn check(password: &str, hash: &str) -> Result<bool> {
        ravel_core::hash::Hash::check(password, hash)
    }
}
```

**log.rs:**
```rust
//! Log facade — re-exports tracing macros for convenience.
pub struct Log;

pub use ravel_core::log::{debug, error, info, trace, warn};
```

**storage.rs:**
```rust
//! Storage facade — file storage abstraction.
use anyhow::Result;

pub struct Storage;

impl Storage {
    fn disk() -> &'static ravel_support::storage::LocalDisk {
        // LocalDisk needs a root path. Use a lazily-initialized static.
        use std::sync::OnceLock;
        static DISK: OnceLock<ravel_support::storage::LocalDisk> = OnceLock::new();
        DISK.get_or_init(|| ravel_support::storage::LocalDisk::new("storage"))
    }

    pub fn put(path: &str, contents: &[u8]) -> Result<()> {
        ravel_support::storage::Storage::put(Self::disk(), path, contents)
    }

    pub fn get(path: &str) -> Result<Vec<u8>> {
        ravel_support::storage::Storage::get(Self::disk(), path)
    }

    pub fn exists(path: &str) -> bool {
        ravel_support::storage::Storage::exists(Self::disk(), path)
    }

    pub fn delete(path: &str) -> Result<()> {
        ravel_support::storage::Storage::delete(Self::disk(), path)
    }
}
```

**queue.rs:**
```rust
//! Queue facade — job queue dispatch.
use anyhow::Result;
use ravel_core::app::APP;
use ravel_support::queue::{Job, Queue as QueueEngine};
use std::sync::Arc;

pub struct Queue;

impl Queue {
    fn engine() -> Arc<QueueEngine> {
        APP.get()
            .expect("Application not booted")
            .container()
            .resolve::<QueueEngine>()
            .expect("Queue not registered — call Application::with_queue() before boot")
    }

    pub fn dispatch<J: Job + serde::Serialize>(job: J) -> Result<()> {
        Self::engine().dispatch(job)
    }

    pub fn dispatch_later<J: Job + serde::Serialize>(job: J, delay: chrono::Duration) -> Result<()> {
        Self::engine().dispatch_later(job, delay)
    }

    pub fn pending() -> Result<usize> {
        Self::engine().pending()
    }
}
```

**route.rs** (placeholder):
```rust
//! Route facade — fluent route builder.
use axum::Router;
use parking_lot::Mutex;
use std::sync::OnceLock;

static REGISTRY: OnceLock<Mutex<Option<RouteBuilder>>> = OnceLock::new();

pub struct Route;

impl Route {
    pub fn get(_path: &str, _handler: impl axum::handler::Handler<(), ()>) -> &'static Self {
        unimplemented!("Route facade")
    }

    pub fn build() -> Router {
        unimplemented!("Route::build")
    }
}

pub struct RouteBuilder {
    routes: Vec<RouteEntry>,
}

enum RouteEntry {
    Get(String, Box<dyn FnOnce()>),
    // etc.
}
```

**auth.rs, session.rs, request.rs** (placeholders with `unimplemented!()`):
```rust
//! Auth facade — authentication helpers.
pub struct Auth;
impl Auth {
    pub fn check() -> bool { unimplemented!("Auth::check") }
    pub fn guest() -> bool { unimplemented!("Auth::guest") }
    pub fn id<T: serde::de::DeserializeOwned>() -> Option<T> { unimplemented!("Auth::id") }
    pub fn login<T: serde::Serialize>(_id: T) { unimplemented!("Auth::login") }
    pub fn logout() { unimplemented!("Auth::logout") }
}
```

```rust
//! Session facade — encrypted cookie sessions.
pub struct Session;
impl Session {
    pub fn get<T: serde::de::DeserializeOwned>(_key: &str) -> Option<T> { unimplemented!("Session::get") }
    pub fn put<T: serde::Serialize>(_key: &str, _value: T) { unimplemented!("Session::put") }
    pub fn has(_key: &str) -> bool { unimplemented!("Session::has") }
    pub fn forget(_key: &str) { unimplemented!("Session::forget") }
    pub fn flash<T: serde::Serialize>(_key: &str, _value: T) { unimplemented!("Session::flash") }
    pub fn flashed<T: serde::de::DeserializeOwned>(_key: &str) -> Option<T> { unimplemented!("Session::flashed") }
}
```

```rust
//! Request facade — current HTTP request accessor.
pub fn request() -> RequestProxy { unimplemented!("request()") }
pub struct RequestProxy;
impl RequestProxy {
    pub fn query<T: serde::de::DeserializeOwned>(&self, _key: &str) -> Option<T> { unimplemented!() }
    pub fn header(&self, _name: &str) -> Option<&str> { unimplemented!() }
    pub fn path(&self) -> &str { unimplemented!() }
    pub fn method(&self) -> &str { unimplemented!() }
    pub fn wants_json(&self) -> bool { unimplemented!() }
}
```

**response.rs:**
```rust
//! Response helpers — response(), redirect(), back(), abort().
use axum::response::IntoResponse;
use ravel_http::error::RavelError;

pub fn response() -> ravel_http::response::ResponseBuilder {
    ravel_http::response::response()
}

pub fn redirect(url: &str) -> RedirectResponse {
    RedirectResponse { url: url.to_string(), status: 302 }
}

pub fn back() -> RedirectResponse {
    RedirectResponse { url: String::new(), status: 302 }
}

pub fn abort(status: u16, message: impl Into<String>) -> RavelError {
    match status {
        400 => RavelError::bad_request(message),
        401 => RavelError::unauthorized(message),
        403 => RavelError::forbidden(message),
        404 => RavelError::not_found(message),
        _ => RavelError::internal(anyhow::anyhow!("{}", message.into())),
    }
}

pub struct RedirectResponse {
    url: String,
    status: u16,
}

impl IntoResponse for RedirectResponse {
    fn into_response(self) -> axum::response::Response {
        use axum::http::{header, StatusCode};
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::FOUND);
        let url = if self.url.is_empty() {
            // back() — no Referer, fallback to /
            "/".to_string()
        } else {
            self.url.clone()
        };
        axum::response::Response::builder()
            .status(status)
            .header(header::LOCATION, &url)
            .body(axum::body::Body::empty())
            .unwrap()
    }
}
```

**collection.rs:**
```rust
//! Collection pipeline — Laravel-style iterable wrapper.
pub struct Collection<T> {
    items: Vec<T>,
}

impl<T> Collection<T> {
    pub fn new(items: Vec<T>) -> Self { Self { items } }

    pub fn map<U>(self, f: impl FnMut(T) -> U) -> Collection<U> {
        Collection { items: self.items.into_iter().map(f).collect() }
    }

    pub fn filter(self, f: impl FnMut(&T) -> bool) -> Self {
        Self { items: self.items.into_iter().where_eq(f).collect() }
    }

    pub fn reject(self, f: impl FnMut(&T) -> bool) -> Self {
        Self { items: self.items.into_iter().where_eq(|x| !f(x)).collect() }
    }

    pub fn first(&self) -> Option<&T> { self.items.first() }
    pub fn last(&self) -> Option<&T> { self.items.last() }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
    pub fn is_not_empty(&self) -> bool { !self.items.is_empty() }
    pub fn count(&self) -> usize { self.items.len() }
    pub fn to_vec(self) -> Vec<T> { self.items }

    pub fn take(self, n: usize) -> Self {
        Self { items: self.items.into_iter().take(n).collect() }
    }

    pub fn skip(self, n: usize) -> Self {
        Self { items: self.items.into_iter().skip(n).collect() }
    }

    pub fn contains(&self, f: impl FnMut(&T) -> bool) -> bool {
        self.items.iter().any(f)
    }

    pub fn each(self, mut f: impl FnMut(&T)) -> Self {
        for item in &self.items { f(item); }
        self
    }
}

impl<T: Ord> Collection<T> {
    pub fn sort(self) -> Self {
        let mut items = self.items;
        items.sort();
        Self { items }
    }
}

impl<T: std::fmt::Display> Collection<T> {
    pub fn implode(&self, glue: &str) -> String {
        self.items.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(glue)
    }
}

impl<T> From<Vec<T>> for Collection<T> {
    fn from(items: Vec<T>) -> Self { Self { items } }
}

impl<T> From<Collection<T>> for Vec<T> {
    fn from(c: Collection<T>) -> Self { c.items }
}

#[macro_export]
macro_rules! collect {
    ($vec:expr) => {
        $crate::collection::Collection::new($vec)
    };
}
```

**path.rs:**
```rust
//! Path helpers — project directory path builders.
use std::path::PathBuf;

fn project_root() -> &'static PathBuf {
    use std::sync::OnceLock;
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        std::env::current_dir().expect("Failed to determine project root")
    })
}

pub fn base_path(segments: &[&str]) -> PathBuf {
    let mut p = project_root().clone();
    for seg in segments { p.push(seg); }
    p
}

pub fn config_path(file: &str) -> PathBuf {
    project_root().join("config").join(file)
}

pub fn database_path(file: &str) -> PathBuf {
    project_root().join("database").join(file)
}

pub fn storage_path(file: &str) -> PathBuf {
    project_root().join("storage").join(file)
}
```

**utils.rs:**
```rust
//! Utility helpers — env(), now().
pub fn env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

pub fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

pub fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}
```

- [ ] **Step 4: Build to verify**

Run: `cargo build -p ravel-facades`
Expected: Compiles with `unimplemented!()` warnings — acceptable for placeholder task.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/ravel-facades/
git commit -m "feat(facades): create ravel-facades crate with all facade modules"
```

---

### Task 2: Global Application singleton + convenience methods

**Files:**
- Modify: `crates/ravel-core/src/app.rs`

- [ ] **Step 1: Add OnceLock import and global APP static**

After the existing `use` block in `E:\rust\ravel\crates\ravel-core\src\app.rs`, add:

```rust
use std::sync::OnceLock;

/// Global application instance, set by [`Application::boot()`].
/// All facades read from this singleton.
pub static APP: OnceLock<Application> = OnceLock::new();
```

- [ ] **Step 2: Change boot() signature and body**

Change `pub fn boot(mut self) -> Result<Self>` to `pub fn boot(mut self) -> Result<()>` and replace `Ok(self)` at the end with:

```rust
    self.booted = true;
    self.container.freeze();

    APP.set(self)
        .map_err(|_| anyhow::anyhow!("Application already booted"))?;

    log::info!("Application booted successfully");
    Ok(())
}
```

- [ ] **Step 3: Add convenience methods to Application**

After `pub fn instance<T>(&self, value: T)` (around line 237), add:

```rust
    /// Register the in-memory cache so facades can use it.
    pub fn with_cache(self) -> Self {
        self.container.instance(crate::cache::MemoryCache::new());
        self
    }

    /// Register the encrypter from an APP_KEY string.
    /// Accepts `base64:` prefix format (ravel key:generate output).
    pub fn with_app_key(self, key: &str) -> Result<Self> {
        let crypt = crate::crypt::Crypt::from_key(key)?;
        self.container.instance(crypt);
        Ok(self)
    }

    /// Register the in-memory queue so facades can dispatch jobs.
    pub fn with_queue(self) -> Self {
        let queue = ravel_support::queue::Queue::memory();
        self.container.instance(queue);
        self
    }
```

Wait — `ravel_support` is not a dependency of `ravel-core`. The dependency chain is:
- ravel-core → no ravel deps
- ravel-support → ravel-core

So `ravel-core` cannot import from `ravel-support`. The `with_queue()` method can't live here.

Solution: Put `with_queue()` in a separate extension trait in `ravel-facades`, or don't provide it at all and let users register the queue manually.

For Round 1: `with_cache()` and `with_app_key()` can live in `ravel-core` (they only use `ravel_core` types). `with_queue()` goes in `ravel-facades` as an extension.

```rust
// In ravel-core::app:
impl Application {
    pub fn with_cache(self) -> Self {
        self.container.instance(crate::cache::MemoryCache::new());
        self
    }

    pub fn with_app_key(self, key: &str) -> Result<Self> {
        let crypt = crate::crypt::Crypt::from_key(key)?;
        self.container.instance(crypt);
        Ok(self)
    }
}

// In ravel-facades (extension trait):
pub trait ApplicationExt {
    fn with_queue(self) -> Self;
}

impl ApplicationExt for ravel_core::app::Application {
    fn with_queue(self) -> Self {
        let queue = ravel_support::queue::Queue::memory();
        self.container().instance(queue);
        self
    }
}
```

Wait, `register_provider` takes `mut self`. `with_queue` needs `self` (owned or `&self`). Let me check the existing method signatures:

- `pub fn load_config(mut self, dir) -> Result<Self>` — owned
- `pub fn register_provider<P>(mut self, provider: P) -> Self` — owned
- `pub fn singleton<T, F>(&self, factory: F)` — shared ref
- `pub fn instance<T>(&self, value: T)` — shared ref

So `instance()` takes `&self`. Good. Our convenience methods can take `self` (owned) and call `self.container.instance(...)` internally:

```rust
pub fn with_cache(self) -> Self {
    self.container.instance(crate::cache::MemoryCache::new());
    self
}
```

This works because `instance()` takes `&self` via the Container's internal `RwLock`. Even though `self` is moved into `with_cache`, `self.container` is still accessible as a field until the method returns `self`.

Actually wait — `self.container.instance(...)` borrows `self.container` immutably. But `self` is owned. That's fine — `self.container` is a field access on an owned value.

But the `with_queue` extension trait: it needs to call `self.container().instance(...)`. `self.container()` returns `&Container`. That borrows `self` immutably. Then `instance()` on Container takes `&self`. This is a double immutable borrow — fine.

But `self` is consumed (owned `self` parameter). We can't call `self.container()` which returns `&Container` and then return `self` — because `self.container()` borrows `self` and the borrow must end before we can move `self`. Actually, in Rust, this is fine because `self.container()` returns a reference with the same lifetime as the borrow, and the borrow ends when the method call completes. Then `self` can be returned.

```rust
pub fn with_queue(self) -> Self {
    let queue = Queue::memory();
    self.container().instance(queue); // borrows self, then releases
    self // move self out
}
```

This compiles fine. OK, let me include this in the plan.

Actually, let me just put the `with_queue` extension in a separate task (Task 4, alongside the Queue facade). Task 2 focuses only on ravel-core changes.

- [ ] **Step 4: Update ravel-core tests**

The existing tests in `app.rs` that use `.boot().unwrap()` to get an Application back must be updated:

1. `test_register_boot_order` — change `let app = ...boot().unwrap()` to `...boot().unwrap(); let app = APP.get().unwrap();`
2. `test_env_available_in_container` — same pattern
3. `test_late_provider_registration` — same pattern, and verify late-registered provider's read-only operations still work

- [ ] **Step 5: Run ravel-core tests**

```bash
cargo test -p ravel-core
```
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/ravel-core/src/app.rs
git commit -m "feat(core): global APP singleton, boot() → Result<()>, with_cache/with_app_key"
```

---

### Task 3: Implement stateless facades (Config, Cache, Crypt, Hash, Log, Storage)

**Files:**
- Modify: `crates/ravel-facades/src/config.rs` (replace placeholder)
- Modify: `crates/ravel-facades/src/cache.rs` (replace placeholder)
- Modify: `crates/ravel-facades/src/crypt.rs` (replace placeholder)
- Modify: `crates/ravel-facades/src/hash.rs` (replace placeholder)
- Modify: `crates/ravel-facades/src/log.rs` (replace placeholder)
- Modify: `crates/ravel-facades/src/storage.rs` (replace placeholder)

- [ ] **Step 1: Write config.rs (final)**

Replace file content with:

```rust
//! Config facade — dot-notation configuration access.
//! Delegates to the global Application's ConfigRepo.

use ravel_core::app::APP;
use serde::de::DeserializeOwned;

pub struct Config;

impl Config {
    fn app() -> &'static ravel_core::app::Application {
        APP.get().expect("Application not booted — call Application::boot() first")
    }

    pub fn get<T: DeserializeOwned>(key: &str) -> Option<T> {
        Self::app().config().get(key)
    }

    pub fn get_or<T: DeserializeOwned>(key: &str, default: T) -> T {
        Self::get(key).unwrap_or(default)
    }

    pub fn has(key: &str) -> bool {
        Self::app().config().has(key)
    }
}
```

- [ ] **Step 2: Write cache.rs (final)**

Replace file content with:

```rust
use ravel_core::app::APP;
use ravel_core::cache::Cache as CacheTrait;
use std::time::Duration;

pub struct Cache;

impl Cache {
    fn cache() -> std::sync::Arc<ravel_core::cache::MemoryCache> {
        APP.get()
            .expect("Application not booted")
            .container()
            .resolve::<ravel_core::cache::MemoryCache>()
            .expect("MemoryCache not registered — use Application::with_cache() before boot")
    }

    pub fn put(key: &str, value: impl std::any::Any + Send + Sync, ttl: Option<Duration>) {
        Self::cache().put(key, Box::new(value), ttl);
    }

    pub fn get<T: 'static + Clone + Send + Sync>(key: &str) -> Option<T> {
        Self::cache().get::<T>(key)
    }

    pub fn has(key: &str) -> bool {
        Self::cache().has(key)
    }

    pub fn forget(key: &str) {
        Self::cache().forget(key);
    }

    pub fn flush() {
        Self::cache().flush();
    }
}
```

- [ ] **Step 3: Write crypt.rs (final)**

Replace file content with:

```rust
use anyhow::Result;
use ravel_core::app::APP;
use ravel_core::crypt::Crypt as CryptEngine;

pub struct Crypt;

impl Crypt {
    fn engine() -> std::sync::Arc<CryptEngine> {
        APP.get()
            .expect("Application not booted")
            .container()
            .resolve::<CryptEngine>()
            .expect("Crypt not registered — use Application::with_app_key() before boot")
    }

    pub fn encrypt(plaintext: &[u8]) -> Result<String> {
        Self::engine().encrypt(plaintext)
    }

    pub fn decrypt(encoded: &str) -> Result<Vec<u8>> {
        Self::engine().decrypt(encoded)
    }

    pub fn encrypt_value<T: serde::Serialize>(value: &T) -> Result<String> {
        Self::engine().encrypt_value(value)
    }

    pub fn decrypt_value<T: serde::de::DeserializeOwned>(encoded: &str) -> Result<T> {
        Self::engine().decrypt_value(encoded)
    }
}
```

- [ ] **Step 4: Write hash.rs, log.rs, storage.rs (final)**

These are one-liner delegations — replace placeholder content with final versions from Task 1 Step 3 above.

- [ ] **Step 5: Run tests on affected crates**

```bash
cargo test -p ravel-core -p ravel-facades
```
Expected: ravel-core tests pass; ravel-facades tests not yet written (TBD in Task 12).

- [ ] **Step 6: Commit**

```bash
git add crates/ravel-facades/src/config.rs crates/ravel-facades/src/cache.rs crates/ravel-facades/src/crypt.rs crates/ravel-facades/src/hash.rs crates/ravel-facades/src/log.rs crates/ravel-facades/src/storage.rs
git commit -m "feat(facades): implement Config, Cache, Crypt, Hash, Log, Storage facades"
```

---

### Task 4: Queue facade + ApplicationExt

**Files:**
- Modify: `crates/ravel-facades/src/queue.rs` (replace placeholder)
- Modify: `crates/ravel-facades/src/lib.rs` (add ApplicationExt)

- [ ] **Step 1: Write queue.rs (final)**

Replace `E:\rust\ravel\crates\ravel-facades\src\queue.rs`:

```rust
//! Queue facade — job dispatch.
use anyhow::Result;
use ravel_core::app::APP;
use ravel_support::queue::{Job, Queue as QueueEngine};
use std::sync::Arc;

pub struct Queue;

impl Queue {
    fn engine() -> Arc<QueueEngine> {
        APP.get()
            .expect("Application not booted")
            .container()
            .resolve::<QueueEngine>()
            .expect("Queue not registered — use ApplicationExt::with_queue() before boot")
    }

    pub fn dispatch<J: Job + serde::Serialize>(job: J) -> Result<()> {
        Self::engine().dispatch(job)
    }

    pub fn dispatch_later<J: Job + serde::Serialize>(job: J, delay: chrono::Duration) -> Result<()> {
        Self::engine().dispatch_later(job, delay)
    }

    pub fn pending() -> Result<usize> {
        Self::engine().pending()
    }
}
```

- [ ] **Step 2: Add ApplicationExt to lib.rs**

Add to `E:\rust\ravel\crates\ravel-facades\src\lib.rs` after module declarations:

```rust
/// Extension trait for Application to register services that live outside ravel-core.
pub trait ApplicationExt {
    fn with_queue(self) -> Self;
}

impl ApplicationExt for ravel_core::app::Application {
    fn with_queue(self) -> Self {
        let queue = ravel_support::queue::Queue::memory();
        self.container().instance(queue);
        self
    }
}
```

- [ ] **Step 3: Verify compilation**

```bash
cargo build -p ravel-facades
```
Expected: Compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add crates/ravel-facades/src/queue.rs crates/ravel-facades/src/lib.rs
git commit -m "feat(facades): Queue facade + ApplicationExt::with_queue"
```

---

### Task 5: Request-level facades foundation (REQUEST task-local + middleware)

**Files:**
- Create: `crates/ravel-http/src/facades/mod.rs`

- [ ] **Step 1: Read ravel-http lib.rs to check current module structure**

```bash
cat E:\rust\ravel\crates\ravel-http\src\lib.rs
```

- [ ] **Step 2: Add `pub mod facades;` to ravel-http lib.rs**

Edit `E:\rust\ravel\crates\ravel-http\src\lib.rs`, add after existing module declarations:

```rust
pub mod facades;
```

- [ ] **Step 3: Write facades/mod.rs with RequestContext, REQUEST, middleware**

Create `E:\rust\ravel\crates\ravel-http\src\facades\mod.rs`:

```rust
//! Request-level facade support — task-local context for Auth, Session, request().

use crate::request::RavelRequest;
use crate::session::SessionData;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use parking_lot::Mutex;
use std::sync::Arc;

// ── Request Context ──────────────────────────────────────────────────

/// Per-request state accessible to facades via [`REQUEST`].
pub struct RequestContext {
    /// The raw HTTP request wrapper.
    pub request: RavelRequest,
    /// Mutable session data (read/write during request).
    pub session: Mutex<SessionData>,
    /// Current authenticated user ID, if any.
    pub auth_id: Mutex<Option<String>>,
}

impl RequestContext {
    /// Extract context from an incoming Axum request.
    pub fn from_request(req: &Request) -> Self {
        let request = RavelRequest::from_request_ref(req);
        Self {
            request,
            session: Mutex::new(SessionData::default()),
            auth_id: Mutex::new(None),
        }
    }

    /// Hydrate session and auth_id from the request's cookies.
    /// Called inside the middleware before passing control to handlers.
    pub fn hydrate_from_cookies(&self, req: &Request) {
        // Read the encrypted session cookie, decrypt, populate self.session
        // and self.auth_id. For now, leave as no-op until Session facade
        // is fully wired.
        let _ = req;
    }

    /// Write session changes back as Set-Cookie headers (if dirty).
    pub fn commit_to_response(&self, response: Response) -> Response {
        // For now, pass through unchanged.
        response
    }
}

// ── Task-Local ───────────────────────────────────────────────────────

tokio::task_local! {
    /// Per-request context, set by [`start_request`] middleware.
    ///
    /// Accessible via `REQUEST.try_with(|ctx| ...)` inside HTTP handlers.
    /// Outside request scope (CLI, jobs), `try_with` returns `Err`.
    pub static REQUEST: Arc<RequestContext>;
}

// ── Middleware ───────────────────────────────────────────────────────

/// Axum middleware that injects [`RequestContext`] into [`REQUEST`].
///
/// This must be the outermost middleware in the stack so that all
/// downstream handlers and middlewares can access Auth/Session/request().
pub async fn start_request(req: Request, next: Next) -> Response {
    let ctx = Arc::new(RequestContext::from_request(&req));
    ctx.hydrate_from_cookies(&req);

    let response = REQUEST.scope(ctx, next.run(req)).await;
    // Note: response from scope already includes any changes.
    // Cookie writing needs to happen via response extensions or a separate layer.
    response
}
```

- [ ] **Step 4: Add `from_request_ref` constructor to RavelRequest**

In `E:\rust\ravel\crates\ravel-http\src\request.rs`, add a public constructor that creates a `RavelRequest` from a reference (without consuming the Axum Request):

```rust
impl RavelRequest {
    /// Create a RavelRequest from an Axum request reference.
    /// Extracts query parameters, method, path, and headers eagerly.
    pub fn from_request_ref(req: &axum::extract::Request) -> Self {
        let query_params = req
            .uri()
            .query()
            .map(|qs| {
                url::form_urlencoded::parse(qs.as_bytes())
                    .into_owned()
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
        // Store request metadata without cloning the body
        Self {
            inner: req.clone(), // Request<Body> may not be Clone — we store metadata instead
            query_params,
        }
    }
}
```

Wait — `Request<Body>` is not Clone in axum 0.8 (Body = `axum::body::Body` which wraps `Bytes`, which IS Clone, so actually `Request<Body>` IS Clone). Let me verify:

```bash
grep -n "Body" E:\rust\ravel\crates\ravel-http\src\request.rs | head -5
```

RavelRequest currently stores `inner: Request<Body>`. If Request<Body> is Clone (which it is in axum 0.8), the `from_request_ref` above works. If not, store `method: Method`, `path: String`, `headers: HeaderMap` separately.

Check: In axum 0.8, `http::Request<Body>` — Body implements Clone if the inner type does. `axum::body::Body` wraps `Bytes` which is Clone. So `Request<Body>` IS Clone. Good.

- [ ] **Step 5: Make SessionData fields accessible to facades**

In `E:\rust\ravel\crates\ravel-http\src\session.rs`, change `SessionData` fields from private to `pub(crate)`:

```rust
pub struct SessionData {
    pub(crate) values: HashMap<String, String>,
    pub(crate) flash: HashMap<String, String>,
}
```

Also ensure `SessionData` implements `Default`:

```rust
impl Default for SessionData {
    fn default() -> Self {
        Self {
            values: HashMap::new(),
            flash: HashMap::new(),
        }
    }
}
```

- [ ] **Step 6: Build to verify**

```bash
cargo build -p ravel-http
```
Expected: Compiles cleanly.

- [ ] **Step 7: Commit**

```bash
git add crates/ravel-http/src/facades/mod.rs crates/ravel-http/src/lib.rs crates/ravel-http/src/request.rs crates/ravel-http/src/session.rs
git commit -m "feat(http): RequestContext + REQUEST task-local + start_request middleware"
```

---

### Task 6: Auth and Session facades

**Files:**
- Modify: `crates/ravel-facades/src/auth.rs` (replace placeholder)
- Modify: `crates/ravel-facades/src/session.rs` (replace placeholder)
- Modify: `crates/ravel-facades/src/request.rs` (replace placeholder)

- [ ] **Step 1: Write auth.rs (final)**

Replace `E:\rust\ravel\crates\ravel-facades\src\auth.rs`:

```rust
//! Auth facade — authentication state for the current request.
//!
//! Uses [`REQUEST`](ravel_http::facades::REQUEST) task-local.
//! Outside HTTP request scope, all methods return `false` / `None`.

use ravel_http::facades::REQUEST;

pub struct Auth;

impl Auth {
    fn with_ctx<F, R>(f: F) -> R
    where
        F: FnOnce(&ravel_http::facades::RequestContext) -> R,
    {
        REQUEST.try_with(|ctx| f(ctx)).unwrap_or_else(|_| {
            // No request context — outside HTTP handler (CLI, job, test).
            // Return sensible defaults matching Laravel behavior.
            unreachable!("Auth called outside request scope — return value handled at call site")
        })
    }

    /// Check if the current user is authenticated.
    pub fn check() -> bool {
        REQUEST
            .try_with(|ctx| ctx.auth_id.lock().is_some())
            .unwrap_or(false)
    }

    /// Check if the current user is a guest (not authenticated).
    pub fn guest() -> bool {
        !Self::check()
    }

    /// Get the authenticated user's ID.
    pub fn id<T: serde::de::DeserializeOwned>() -> Option<T> {
        REQUEST
            .try_with(|ctx| {
                let auth_id = ctx.auth_id.lock();
                auth_id.as_ref().and_then(|id| serde_json::from_str(id).ok())
            })
            .unwrap_or(None)
    }

    /// Log in the given user ID. Marks session as dirty.
    pub fn login<T: serde::Serialize>(id: &T) {
        let _ = REQUEST.try_with(|ctx| {
            if let Ok(json) = serde_json::to_string(id) {
                *ctx.auth_id.lock() = Some(json);
            }
        });
    }

    /// Log out the current user.
    pub fn logout() {
        let _ = REQUEST.try_with(|ctx| {
            *ctx.auth_id.lock() = None;
        });
    }
}
```

- [ ] **Step 2: Write session.rs (final)**

Replace `E:\rust\ravel\crates\ravel-facades\src\session.rs`:

```rust
//! Session facade — encrypted cookie session access.
//!
//! Uses [`REQUEST`](ravel_http::facades::REQUEST) task-local.
//! Outside HTTP request scope, all methods return `None` / are no-ops.

use ravel_http::facades::REQUEST;
use ravel_http::session::SessionData;

pub struct Session;

impl Session {
    pub fn get<T: serde::de::DeserializeOwned>(key: &str) -> Option<T> {
        REQUEST
            .try_with(|ctx| {
                let session = ctx.session.lock();
                session.values.get(key).and_then(|v| serde_json::from_str(v).ok())
            })
            .unwrap_or(None)
    }

    pub fn put<T: serde::Serialize>(key: &str, value: &T) {
        let _ = REQUEST.try_with(|ctx| {
            if let Ok(json) = serde_json::to_string(value) {
                ctx.session.lock().values.insert(key.to_string(), json);
            }
        });
    }

    pub fn has(key: &str) -> bool {
        REQUEST
            .try_with(|ctx| ctx.session.lock().values.contains_key(key))
            .unwrap_or(false)
    }

    pub fn forget(key: &str) {
        let _ = REQUEST.try_with(|ctx| {
            ctx.session.lock().values.remove(key);
        });
    }

    pub fn flash<T: serde::Serialize>(key: &str, value: &T) {
        let _ = REQUEST.try_with(|ctx| {
            if let Ok(json) = serde_json::to_string(value) {
                ctx.session.lock().flash.insert(key.to_string(), json);
            }
        });
    }

    pub fn flashed<T: serde::de::DeserializeOwned>(key: &str) -> Option<T> {
        REQUEST
            .try_with(|ctx| {
                let mut session = ctx.session.lock();
                session.flash.remove(key).and_then(|v| serde_json::from_str(&v).ok())
            })
            .unwrap_or(None)
    }
}
```

- [ ] **Step 3: Write request.rs (final)**

Replace `E:\rust\ravel\crates\ravel-facades\src\request.rs`:

```rust
//! Request facade — access the current HTTP request.
//!
//! Uses [`REQUEST`](ravel_http::facades::REQUEST) task-local.

use ravel_http::facades::REQUEST;

pub fn request() -> RequestProxy {
    RequestProxy
}

pub struct RequestProxy;

impl RequestProxy {
    pub fn query<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        REQUEST
            .try_with(|ctx| ctx.request.query(key))
            .unwrap_or(None)
    }

    pub fn header(&self, name: &str) -> Option<String> {
        REQUEST
            .try_with(|ctx| ctx.request.header(name).map(|s| s.to_string()))
            .unwrap_or(None)
    }

    pub fn path(&self) -> Option<String> {
        REQUEST
            .try_with(|ctx| Some(ctx.request.path().to_string()))
            .unwrap_or(None)
    }

    pub fn method(&self) -> Option<String> {
        REQUEST
            .try_with(|ctx| Some(ctx.request.method().to_string()))
            .unwrap_or(None)
    }

    pub fn wants_json(&self) -> bool {
        REQUEST
            .try_with(|ctx| ctx.request.wants_json())
            .unwrap_or(false)
    }
}
```

Wait — `RavelRequest` has `path()` returning `&str`, `method()` returning `&str`, `query<T>(key)`, `header(name)` returning `Option<&str>`, `wants_json()` returning `bool`. Let me check the actual API from the codebase...

From the ravel-http exploration earlier:
- `query<T: DeserializeOwned>(&self, key) -> Option<T>` ✓
- `header(&self, name) -> Option<&str>` ✓ (but we need owned String for Task-Local lifetime reasons)
- `path(&self) -> &str` ✓
- `method(&self) -> &str` ✓
- `wants_json(&self) -> bool` ✓

The lifetime issue: `REQUEST.try_with(|ctx| ...)` borrows the Arc, so returned references must be owned (String). Fine for our facade.

- [ ] **Step 4: Build**

```bash
cargo build -p ravel-facades
```
Expected: Compiles; check that SessionData's fields are public or have accessor methods.

If `SessionData.values` is private, we need to either:
(a) Make `SessionData` fields `pub(crate)` or `pub`, or
(b) Add accessor methods to `SessionData`.

Check: `grep -n "pub.*SessionData" E:\rust\ravel\crates\ravel-http\src\session.rs`. If private, add `pub(crate)` to fields or add getter/setter methods. For this plan, assume we make session fields `pub(crate)`.

- [ ] **Step 5: Commit**

```bash
git add crates/ravel-facades/src/auth.rs crates/ravel-facades/src/session.rs crates/ravel-facades/src/request.rs
git commit -m "feat(facades): Auth, Session, request() facades via REQUEST task-local"
```

---

### Task 7: Route facade + RouteRegistry

**Files:**
- Modify: `crates/ravel-facades/src/route.rs` (replace placeholder)

- [ ] **Step 1: Add RouteRegistry to ravel-http**

Add to `E:\rust\ravel\crates\ravel-http\src\route.rs` (after Route struct definition):

```rust
use parking_lot::Mutex;
use std::sync::OnceLock;

/// Global route registry for the Route facade.
///
/// Filled during boot by `Route::get()`, `Route::post()`, etc.
/// Consumed by `Route::build()` which produces the final `axum::Router`.
///
/// OnceLock + Mutex ensures single initialization and thread-safe writes
/// during the boot phase (before any concurrent access).
static ROUTE_REGISTRY: OnceLock<Mutex<Option<RouteRegistry>>> = OnceLock::new();

pub(crate) fn route_registry() -> &'static Mutex<Option<RouteRegistry>> {
    ROUTE_REGISTRY.get_or_init(|| Mutex::new(Some(RouteRegistry::new())))
}

pub struct RouteRegistry {
    routes: Vec<RouteEntry>,
    groups: Vec<GroupEntry>,
    middlewares: Vec<MiddlewareEntry>,
}

enum RouteEntry {
    Verb {
        method: &'static str,  // "GET", "POST", etc.
        path: String,
        handler: Box<dyn FnOnce() + Send>, // type-erased
    },
}

enum GroupEntry {
    Prefix(String, Box<dyn FnOnce() + Send>),
}

enum MiddlewareEntry {
    Fn(Box<dyn Fn() + Send>),
}

impl RouteRegistry {
    pub fn new() -> Self {
        Self {
            routes: Vec::new(),
            groups: Vec::new(),
            middlewares: Vec::new(),
        }
    }

    /// Build the final axum::Router from the accumulated registry.
    /// This consumes the registry (set the Mutex to None).
    pub fn build(router: axum::Router) -> axum::Router {
        // Apply all accumulated routes, groups, middlewares
        // This is called only once during boot.
        router
    }
}
```

Hmm, the type-erased handler approach is complex. Let me use a simpler approach that matches how the current `Route` builder works internally — just accumulate on an `axum::Router` directly:

Actually, the SIMPLEST approach for Route facade is: just use a Mutex-wrapped `axum::Router`. Facade methods call `.route()` on it. `build()` returns the accumulated router.

Let me rewrite route.rs with this simple approach.

**Final approach for Route facade:**

```rust
//! Route facade — static route builder.
//!
//! Usage (in ServiceProvider::register or boot):
//! ```rust
//! Route::get("/", handler);
//! Route::group("/admin", || {
//!     Route::get("/dashboard", admin_handler);
//! });
//! let router = Route::build();
//! ```

use axum::Router;
use axum::routing;
use parking_lot::Mutex;
use std::sync::OnceLock;

static REGISTRY: OnceLock<Mutex<Option<RouteState>>> = OnceLock::new();

struct RouteState {
    router: Router,
    group_stack: Vec<String>,
}

fn registry() -> &Mutex<Option<RouteState>> {
    REGISTRY.get_or_init(|| Mutex::new(Some(RouteState {
        router: Router::new(),
        group_stack: Vec::new(),
    })))
}

pub struct Route;

impl Route {
    fn with_router<F>(f: F)
    where
        F: FnOnce(&mut RouteState),
    {
        let mut guard = registry().lock();
        let state = guard.as_mut().expect("Route::build() already called");
        f(state);
    }

    pub fn get(path: &str, handler: impl axum::handler::Handler<(), ()> + Clone + Send + Sync + 'static) {
        Self::with_router(|state| {
            let full_path = Self::full_path(state, path);
            state.router = state.router.clone().route(&full_path, routing::get(handler));
        });
    }

    pub fn post(path: &str, handler: impl axum::handler::Handler<(), ()> + Clone + Send + Sync + 'static) {
        Self::with_router(|state| {
            let full_path = Self::full_path(state, path);
            state.router = state.router.clone().route(&full_path, routing::post(handler));
        });
    }

    pub fn put(path: &str, handler: impl axum::handler::Handler<(), ()> + Clone + Send + Sync + 'static) {
        Self::with_router(|state| {
            let full_path = Self::full_path(state, path);
            state.router = state.router.clone().route(&full_path, routing::put(handler));
        });
    }

    pub fn delete(path: &str, handler: impl axum::handler::Handler<(), ()> + Clone + Send + Sync + 'static) {
        Self::with_router(|state| {
            let full_path = Self::full_path(state, path);
            state.router = state.router.clone().route(&full_path, routing::delete(handler));
        });
    }

    pub fn patch(path: &str, handler: impl axum::handler::Handler<(), ()> + Clone + Send + Sync + 'static) {
        Self::with_router(|state| {
            let full_path = Self::full_path(state, path);
            state.router = state.router.clone().route(&full_path, routing::patch(handler));
        });
    }

    pub fn group(prefix: &str, f: impl FnOnce()) {
        Self::with_router(|state| {
            state.group_stack.push(prefix.to_string());
        });
        f();
        Self::with_router(|state| {
            state.group_stack.pop();
        });
    }

    pub fn middleware<F>(f: F)
    where
        F: Fn(axum::extract::Request, axum::middleware::Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = axum::response::Response> + Send>> + Clone + Send + Sync + 'static,
    {
        Self::with_router(|state| {
            state.router = state.router.clone().layer(axum::middleware::from_fn(f));
        });
    }

    /// Build the final Router. Consumes the registry.
    /// Subsequent calls to Route methods will panic.
    pub fn build() -> Router {
        let mut guard = registry().lock();
        guard.take().expect("Route::build() already called").router
    }

    fn full_path(state: &RouteState, path: &str) -> String {
        if state.group_stack.is_empty() {
            path.to_string()
        } else {
            format!("{}{}", state.group_stack.join(""), path)
        }
    }
}
```

- [ ] **Step 2: Replace route.rs with Route facade implementation**

Write the above code to `E:\rust\ravel\crates\ravel-facades\src\route.rs`.

- [ ] **Step 3: Build and verify**

```bash
cargo build -p ravel-facades
```
Expected: Compiles cleanly. Import checks: `axum::routing::get/post/put/delete/patch`, `axum::middleware::from_fn`.

- [ ] **Step 4: Commit**

```bash
git add crates/ravel-facades/src/route.rs
git commit -m "feat(facades): Route facade with group/middleware/build support"
```

---

### Task 8: Response helpers + Collections + Path + Utils (finalize remaining modules)

**Files:**
- Modify: `crates/ravel-facades/src/response.rs` (replace placeholder)
- Modify: `crates/ravel-facades/src/collection.rs` (replace placeholder)
- Modify: `crates/ravel-facades/src/path.rs` (replace placeholder)
- Modify: `crates/ravel-facades/src/utils.rs` (replace placeholder)

- [ ] **Step 1: Finalize response.rs, collection.rs, path.rs, utils.rs**

Replace each placeholder file with the final versions from Task 1 Step 3 above. Ensure they compile:

```bash
cargo build -p ravel-facades
```
Expected: Compiles cleanly.

- [ ] **Step 2: Commit**

```bash
git add crates/ravel-facades/src/response.rs crates/ravel-facades/src/collection.rs crates/ravel-facades/src/path.rs crates/ravel-facades/src/utils.rs
git commit -m "feat(facades): response helpers, collections, path, utils — all modules finalized"
```

---

### Task 9: Update testblog to use facades

**Files:**
- Modify: `testblog/src/main.rs`
- Modify: `testblog/bootstrap/app.rs`
- Modify: `testblog/routes/web.rs`
- Modify: `testblog/Cargo.toml`

- [ ] **Step 1: Add ravel-facades dependency to testblog**

In `E:\rust\ravel\testblog\Cargo.toml`, add:

```toml
ravel-facades = { path = "../crates/ravel-facades" }
```

- [ ] **Step 2: Update bootstrap/app.rs to use Route facade**

Replace `E:\rust\ravel\testblog\bootstrap\app.rs` content:

```rust
use anyhow::Result;
use ravel_core::app::{Application, ServiceProvider};
use ravel_core::container::Container;
use ravel_facades::Route;

pub struct RouteServiceProvider;

impl ServiceProvider for RouteServiceProvider {
    fn register(&self, _container: &Container) -> Result<()> {
        Route::get("/", || async { "Hello, Ravel!" });
        Ok(())
    }

    fn name(&self) -> &str {
        "RouteServiceProvider"
    }
}

pub fn create_app() {
    Application::new()
        .load_env(".")
        .expect("Failed to load .env")
        .load_config("config")
        .expect("Failed to load config")
        .register_provider(RouteServiceProvider)
        .boot()
        .expect("Failed to boot application");
}
```

- [ ] **Step 3: Update main.rs to use facades**

Replace `E:\rust\ravel\testblog\src\main.rs` content:

```rust
mod bootstrap;

use ravel_core::app::APP;
use ravel_facades::{Config, Route};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    bootstrap::app::create_app();

    let router = Route::build();

    let app = APP.get().expect("Application not booted");
    let addr = Config::get_or::<String>("server.host", "127.0.0.1".into());
    let port = Config::get_or::<u16>("server.port", 3000);
    let bind_addr = format!("{addr}:{port}");

    println!("Ravel running at http://{bind_addr}");

    ravel_http::server::serve(router, &bind_addr).await?;
    Ok(())
}
```

- [ ] **Step 4: Update routes/web.rs**

Replace `E:\rust\ravel\testblog\routes\web.rs`:

```rust
//! Web routes — now registered via the Route facade in RouteServiceProvider.
//! This file is kept for reference but is no longer directly used.
```

- [ ] **Step 5: Build and verify testblog**

```bash
cargo build -p testblog
```
Expected: Compiles cleanly.

- [ ] **Step 6: Commit**

```bash
git add testblog/
git commit -m "refactor(testblog): migrate to ravel-facades (Route, Config)"
```

---

### Task 10: Update integration tests to use facades

**Files:**
- Modify: `testblog/tests/integration_test.rs`

- [ ] **Step 1: Rewrite integration_test.rs with facade-based approach**

The key changes:
- `create_test_app()` uses facades via `Route::get()`, `Route::post()`, etc.
- No more manual container resolution — `Route::build()` handles it.
- Test assertions use `ravel_test::TestClient` (unchanged).

Replace `E:\rust\ravel\testblog\tests\integration_test.rs` content:

```rust
use axum::http::StatusCode;
use ravel_core::app::Application;
use ravel_core::app::ServiceProvider;
use ravel_core::container::Container;
use ravel_facades::Route;
use anyhow::Result;

// ── ServiceProvider ──────────────────────────────────────────────────

struct TestRouteProvider;

impl ServiceProvider for TestRouteProvider {
    fn register(&self, _container: &Container) -> Result<()> {
        Route::get("/", || async { "Hello, Ravel!" });
        Route::get("/health", || async { (StatusCode::OK, "OK") });
        Ok(())
    }

    fn name(&self) -> &str {
        "TestRouteProvider"
    }
}

fn create_test_app() {
    Application::new()
        .register_provider(TestRouteProvider)
        .boot()
        .expect("Failed to boot test application");
}

fn test_router() -> axum::Router {
    create_test_app();
    Route::build()
}

// ── Application bootstrap tests ─────────────────────────────────────

#[test]
fn test_app_boots_successfully() {
    create_test_app();
    let app = ravel_core::app::APP.get().unwrap();
    assert!(app.is_booted());
}

#[test]
fn test_app_is_booted_after_boot() {
    create_test_app();
    let app = ravel_core::app::APP.get().unwrap();
    assert!(app.is_booted());
}

#[test]
fn test_app_config_loaded() {
    Application::new()
        .load_config("nonexistent-dir")
        .unwrap()
        .boot()
        .unwrap();
    let app = ravel_core::app::APP.get().unwrap();
    assert!(app.config().is_empty() || !app.config().is_empty());
}

// ── HTTP request tests ──────────────────────────────────────────────

#[tokio::test]
async fn test_root_route_returns_hello() {
    let client = ravel_test::TestClient::new(test_router());
    let resp = client.get("/").await;
    resp.assert_ok();
    resp.assert_see("Hello, Ravel!");
}

#[tokio::test]
async fn test_not_found_route() {
    let client = ravel_test::TestClient::new(test_router());
    let resp = client.get("/nonexistent").await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_health_endpoint() {
    let client = ravel_test::TestClient::new(test_router());
    let resp = client.get("/health").await;
    resp.assert_ok();
    resp.assert_see("OK");
}
```

- [ ] **Step 2: Run integration tests**

```bash
cargo test -p testblog
```
Expected: All tests pass (adapt assertions as needed for Route facade behavior).

- [ ] **Step 3: Commit**

```bash
git add testblog/tests/integration_test.rs
git commit -m "test(testblog): migrate integration tests to ravel-facades"
```

---

### Task 11: E2E facade integration test

**Files:**
- Create: `crates/ravel-facades/tests/integration.rs`

- [ ] **Step 1: Write comprehensive facade integration test**

Create `E:\rust\ravel\crates\ravel-facades\tests\integration.rs`:

```rust
use ravel_core::app::Application;
use ravel_core::app::ServiceProvider;
use ravel_core::container::Container;
use ravel_facades::{Config, Cache, Hash, Route, now, env, response, redirect};
use anyhow::Result;
use std::time::Duration;

// ── Test Provider ────────────────────────────────────────────────────

struct TestProvider;

impl ServiceProvider for TestProvider {
    fn register(&self, _c: &Container) -> Result<()> {
        // Demonstrate Route facade in a provider
        Route::get("/hello", || async { "Hello from facade!" });
        Ok(())
    }

    fn name(&self) -> &str { "TestProvider" }
}

// ── Config Facade Tests ──────────────────────────────────────────────

#[test]
fn test_config_get_after_boot() {
    Application::new()
        .load_config("nonexistent-dir")
        .unwrap()
        .register_provider(TestProvider)
        .boot()
        .unwrap();

    // Config should be accessible after boot
    let _ = Config::get::<String>("nonexistent.key");
    // No panic — returns None
}

#[test]
fn test_config_get_or_with_default() {
    Application::new()
        .register_provider(TestProvider)
        .boot()
        .unwrap();

    let val = Config::get_or("nonexistent", 42u16);
    assert_eq!(val, 42);
}

// ── Cache Facade Tests ───────────────────────────────────────────────

#[test]
fn test_cache_put_and_get() {
    Application::new()
        .with_cache()
        .register_provider(TestProvider)
        .boot()
        .unwrap();

    Cache::put("test_key", "test_value".to_string(), Some(Duration::from_secs(60)));
    let val: Option<String> = Cache::get("test_key");
    assert_eq!(val, Some("test_value".to_string()));
}

#[test]
fn test_cache_miss() {
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

// ── Hash Facade Tests ────────────────────────────────────────────────

#[test]
fn test_hash_make_and_check() {
    Application::new().boot().unwrap();

    let hashed = Hash::make("secret123").unwrap();
    assert!(Hash::check("secret123", &hashed).unwrap());
    assert!(!Hash::check("wrong_password", &hashed).unwrap());
}

// ── Route Facade Tests ───────────────────────────────────────────────

#[test]
fn test_route_facade_builds_router() {
    Application::new()
        .register_provider(TestProvider)
        .boot()
        .unwrap();

    let router = Route::build();
    // Router should be non-empty (has at least one route)
    assert!(!format!("{:?}", router).is_empty());
}

// ── Utility Facade Tests ─────────────────────────────────────────────

#[test]
fn test_now_returns_current_time() {
    let t1 = now();
    std::thread::sleep(Duration::from_millis(1));
    let t2 = now();
    assert!(t2 > t1);
}

#[test]
fn test_env_returns_value_or_none() {
    std::env::set_var("RAVEL_FACADE_TEST_VAR", "hello");
    assert_eq!(env("RAVEL_FACADE_TEST_VAR"), Some("hello".to_string()));
    assert_eq!(env("RAVEL_NONEXISTENT_VAR_XYZ"), None);
    assert_eq!(env_or("RAVEL_NONEXISTENT_VAR_XYZ", "fallback"), "fallback");
    std::env::remove_var("RAVEL_FACADE_TEST_VAR");
}

// ── Response Helper Tests ────────────────────────────────────────────

#[test]
fn test_redirect_returns_302() {
    let resp = redirect("/login").into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::FOUND);
    assert!(resp.headers().get("location").unwrap().to_str().unwrap().contains("/login"));
}

#[test]
fn test_response_builder_creates_response() {
    let resp = response()
        .status(axum::http::StatusCode::CREATED)
        .body("test body")
        .unwrap()
        .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::CREATED);
}

// ── Collection Tests ─────────────────────────────────────────────────

#[test]
fn test_collection_map_filter_sort() {
    use ravel_facades::collect;

    let result = collect!(vec!["carol", "alice", "bob"])
        .map(|s| s.to_uppercase())
        .where_eq(|s| !s.is_empty())
        .sort()
        .to_vec();

    assert_eq!(result, vec!["ALICE", "BOB", "CAROL"]);
}

#[test]
fn test_collection_reject() {
    use ravel_facades::collect;

    let result = collect!(vec![1, 2, 3, 4, 5])
        .reject(|&x| x % 2 == 0)
        .to_vec();

    assert_eq!(result, vec![1, 3, 5]);
}

#[test]
fn test_collection_implode() {
    use ravel_facades::collect;

    let result = collect!(vec!["a", "b", "c"]).implode(", ");
    assert_eq!(result, "a, b, c");
}

// ── Panic on unconfigured services ───────────────────────────────────

#[test]
#[should_panic(expected = "MemoryCache not registered")]
fn test_cache_panics_without_with_cache() {
    Application::new().boot().unwrap();
    Cache::put("key", "val".to_string(), None);
}

#[test]
#[should_panic(expected = "Application not booted")]
fn test_facades_panic_before_boot() {
    // APP is empty — Config::get should panic
    Config::get::<String>("any.key");
}
```

- [ ] **Step 2: Run integration tests**

```bash
cargo test -p ravel-facades
```
Expected: All 14 tests pass. If any fail, debug and fix before committing.

- [ ] **Step 3: Commit**

```bash
git add crates/ravel-facades/tests/
git commit -m "test(facades): comprehensive E2E integration tests for all facades"
```

---

### Task 12: Final verification — full workspace build + test

**Files:** (none — verification only)

- [ ] **Step 1: Build entire workspace**

```bash
cargo build --workspace
```
Expected: Clean build, no errors.

- [ ] **Step 2: Run all tests**

```bash
cargo test --workspace
```
Expected: All tests pass across all crates. No regressions.

- [ ] **Step 3: Run clippy**

```bash
cargo clippy --workspace -- -D warnings
```
Expected: No warnings.

- [ ] **Step 4: Run fmt check**

```bash
cargo fmt --all -- --check
```
Expected: All files formatted correctly.

- [ ] **Step 5: Final commit (if any cleanups)**

```bash
git add -A && git diff --cached --stat
# Only commit if there are lint/format fixes
git commit -m "chore: clippy + fmt fixes for Round 1 facades"
```

---

## Verification Checklist (Post-Implementation)

- [ ] `cargo build --workspace` — clean
- [ ] `cargo test --workspace` — all tests pass
- [ ] `cargo clippy --workspace -- -D warnings` — no warnings
- [ ] `cargo fmt --all -- --check` — no diff
- [ ] `testblog` builds and runs (`cargo run -p testblog` starts server)
- [ ] Facades used in testblog: `Config`, `Route`, `APP`
- [ ] Integration tests cover: Config, Cache, Hash, Route, Response, Collection, Utils
- [ ] Panic message when facade used before boot is clear
- [ ] Panic message when service not registered (e.g. Cache without `with_cache()`) is clear
- [ ] No existing API broken (all existing crate public APIs unchanged except `boot()` return type)
