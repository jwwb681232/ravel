# Ravel Laravel-ification Design

**Date:** 2026-06-08
**Status:** Spec — Awaiting Implementation Plan

## Overview

Three-round design to transform Ravel from a Rust web framework that happens to be inspired by Laravel into one that feels like Laravel. Each round targets a different layer:

| Round | Layer | Core Philosophy | Scope |
|-------|-------|-----------------|-------|
| 1 | API | Static Facade + Task-Local request context + Collection pipeline | New facade module + `REQUEST` global |
| 2 | Architecture | Pipeline pattern + auto-DI + Macroable + lifecycle hooks | Extend Container/Application/middleware |
| 3 | Ecosystem | Queue Worker + Eloquent Model + Blade + Notification | New crates + existing crate extensions |

---

## Round 1: API-Level Laravel-ification

**Goal:** Make Ravel code look and feel like Laravel at the call-site level without changing runtime architecture. This is a "syntax sugar coat" over existing machinery.

### 1.1 Global Application Singleton

```rust
// OnceLock<Arc<Application>> — set after boot(), read-only thereafter
static APP: OnceLock<Arc<Application>> = OnceLock::new();
```

- `Application::boot()` sets the singleton and calls `container.freeze()`
- All stateless facades read from this global
- After freeze, the container is thread-safe for concurrent reads
- Testing: re-initialize per test; a `#[ravel_test]` proc-macro handles setup/teardown

### 1.2 Stateless Facades

Facades that need only the global Application (no request context):

| Facade | Delegates to | Status |
|--------|-------------|--------|
| `Hash` | Zero-size struct (already facade-style) | Zero changes needed |
| `Log` | `tracing` macros re-exported (already facade-style) | Zero changes needed |
| `Storage` | Zero-size struct (already facade-style) | Zero changes needed |
| `Config` | `APP.config()` read access + `set()` for runtime/testing overrides | Add delegation layer |
| `Crypt` | `APP.crypt()` — AES-256-GCM encrypt/decrypt | Add delegation layer |
| `Cache` | `APP.cache()` — TTL-aware in-memory cache | Add delegation layer |
| `Queue` | `APP.queue()` — dispatch/dispatch_later/pending | Add delegation layer |

**API target:**

```rust
use ravel::facades::{Config, Cache, Hash, Crypt, Log, Storage, Queue};

Config::get::<String>("app.name");
Config::get_or::<u16>("server.port", 3000);
Cache::put("key", "value", Duration::from_secs(60));
Crypt::encrypt(b"secret")?;
Hash::make("password")?;
Queue::dispatch(SendWelcomeEmail { user_id: 42 })?;
```

### 1.3 Route Facade

**Problem:** Route building is stateful (accumulating routes before final `build()`). Unlike other facades that read a pre-existing service, Route needs to mutate a registry during boot.

**Solution:** Global `RouteRegistry` wrapped in `Mutex`, active only during boot:

```rust
static ROUTE_REGISTRY: OnceLock<Mutex<RouteRegistry>> = OnceLock::new();
```

- `Route::get()` / `Route::post()` etc. write to the registry
- `Route::build()` consumes the registry and produces `axum::Router`
- Calling `Route::build()` a second time returns an error ("Router already built")
- After `boot()`, the registry mutex is dropped (replaced with `None`)
- Route definitions MUST happen in ServiceProvider `register()` or `boot()` — same as Laravel

**API target:**

```rust
use ravel::facades::Route;

Route::get("/users", list_users);
Route::post("/users", create_user);
Route::group("/admin", |_| {
    Route::get("/dashboard", admin_dashboard);
});
Route::middleware(log_requests);

let router = Route::build();
```

### 1.4 Request-Level Facades (Task-Local)

Facades that depend on the current HTTP request: `Auth`, `Session`, `request()`.

**Mechanism:** `tokio::task_local!` + Axum middleware injection:

```rust
tokio::task_local! {
    static REQUEST: Arc<RequestContext>;
}
```

**`RequestContext` struct:**

```rust
pub struct RequestContext {
    pub request: RavelRequest,
    pub session: Mutex<SessionData>,
    pub auth_id: Mutex<Option<String>>,
}
```

**`start_request` middleware (provided by framework):**

```rust
pub async fn start_request(req: Request, next: Next) -> Response {
    let ctx = Arc::new(RequestContext::from_request(&req));
    ctx.hydrate_from_cookie(&req);       // Decrypt session, set auth_id
    let response = REQUEST.scope(ctx.clone(), next.run(req)).await;
    ctx.commit_to_response(response)     // Encrypt & Set-Cookie if dirty
}
```

**API target:**

```rust
use ravel::facades::{Auth, Session};

async fn dashboard() -> impl IntoResponse {
    if Auth::check() {
        format!("Welcome back, {:?}!", Auth::id::<String>())
    } else {
        redirect("/login")
    }
}

Session::put("last_visit", now());
Session::flash("status", "Profile updated");
```

**No-context behavior (CLI / Jobs / Tests outside scope):**

```rust
Auth::check()        // → false (not panic)
Session::get::<T>(k) // → None
request().query("x") // → None
```

Matches Laravel behavior exactly.

### 1.5 Response Helpers

```rust
// redirect() returns a RedirectResponse (implements IntoResponse)
redirect("/login")                              // 302
redirect("/login", 301)                         // 301 permanent
back()                                          // 302 → Referer header
redirect()->back()                              // Same (builder pattern)

// abort() returns RavelError (implements IntoResponse)
abort(404, "Not found")                         // → RavelError::not_found
abort(403)                                      // → RavelError::forbidden
```

Note: `redirect()` returns a `RedirectResponse` struct (not a builder). `back()` is a standalone function, not `redirect()->back()`. Both are `impl IntoResponse`.

### 1.6 Collection Pipeline

**`Collection<T>` wrapper around `Vec<T>`:**

```rust
use ravel::collect;

let names = collect!(vec!["alice", "bob", "carol"])
    .map(|s| s.to_uppercase())
    .reject(|s| s.is_empty())
    .sort()
    .to_vec();
```

**Key methods:** `map`, `filter`, `reject`, `each`, `pluck(|u| &u.field)`, `group_by(|u| u.field)`, `sort`, `sort_by`, `sort_by_desc`, `first`, `last`, `contains`, `is_empty`, `is_not_empty`, `count`, `take`, `skip`, `chunk`, `sum`, `avg`, `min`, `max`, `implode`, `to_json`

**`pluck` and `group_by` use closures (not strings):** `users.pluck(|u| &u.name)` rather than `pluck("name")`. This is type-safe and idiomatic Rust. No reflection needed.

### 1.7 Path Helpers & Utilities

```rust
config_path("app.toml")              // {root}/config/app.toml
database_path("migrations")          // {root}/database/migrations
storage_path("logs/app.log")         // {root}/storage/logs/app.log
env("APP_NAME")                      // Option<&str>
env_or("APP_DEBUG", "false")        // &str
now()                                // chrono::DateTime<Utc>
```

---

## Round 2: Architecture-Level Laravel-ization

**Goal:** Introduce Laravel's core architectural patterns as first-class reusable abstractions, not just call-site syntax sugar.

### 2.1 Pipeline Pattern

A reusable, standalone middleware pipeline — the same pattern that powers Laravel's HTTP middleware, job middleware, and event filtering.

**Core trait:**

```rust
#[async_trait]
pub trait Pipe<T>: Send + Sync
where T: Send,
{
    async fn handle(&self, passable: T, next: Next<T>) -> T;
}

pub struct Next<T> {
    remaining: Vec<Arc<dyn Pipe<T>>>,
    final_handler: Arc<dyn Fn(T) -> Pin<Box<dyn Future<Output = T> + Send>> + Send + Sync>,
}

impl<T: Send> Next<T> {
    pub async fn run(self, passable: T) -> T { ... }
}
```

**Usage:**

```rust
let result = Pipeline::send(data)
    .through(vec![StepA, StepB, StepC])
    .then(|data| final_handler(data))
    .await;
```

- Existing HTTP middleware functions refactored to implement `Pipe<Request>`
- `Route::middleware()` accepts both `Pipe` implementations and raw closures (wrapped automatically)
- Pipeline is reusable for non-HTTP scenarios: job processing, event filtering, etc.

**Onion model:** Each `Pipe::handle()` receives `next: Next<T>` — calling `next.run(data)` passes control to the next pipe in the chain. Identical to Laravel's `$next($request)`.

### 2.2 Container Auto-Resolution

**Problem:** Current `Container::resolve::<T>()` only works for manually registered types. Laravel's container auto-discovers constructor dependencies via reflection. Rust has no runtime reflection.

**Solution:** Compile-time constructor injection via proc-macro.

**`#[derive(Resolvable)]` proc-macro:**

```rust
#[derive(Resolvable)]
struct UserController {
    #[inject] users: Arc<UserService>,
    #[inject] cache: Arc<Cache>,
    #[default] name: String,
}
```

**Generated code:**

```rust
impl Resolvable for UserController {
    fn resolve(container: &Container) -> Result<Self> {
        Ok(Self {
            users: container.resolve::<UserService>()
                .ok_or_else(|| anyhow!("UserService not found"))?,
            cache: container.resolve::<Cache>()
                .ok_or_else(|| anyhow!("Cache not found"))?,
            name: String::default(),
        })
    }
}
```

**Field attributes:**

| Attribute | Behavior | Type Requirement |
|-----------|----------|------------------|
| `#[inject]` | Resolve from container | `T: Send + Sync + 'static` |
| `#[inject(optional)]` | Resolve, `None` if not registered | `Option<T>` |
| `#[inject(trait = "dyn EventHandler")]` | Key-type trait object resolution | `Arc<dyn Trait>` |
| `#[default]` | Fill with `Default::default()` | Must implement `Default` |
| No attribute | Compile error — must annotate | N/A |

**Container integration:** `resolve::<T>()` first checks the existing singleton/instance registry (fast path), then falls back to `T::resolve()` auto-construction for `Resolvable` types. Auto-resolved types are cached as singletons.

**Behavior:** Only works for types registered in the container before `freeze()` — same as Laravel's constraint.

### 2.3 Macroable Extension

**Problem:** Laravel allows `Str::macro('toSlug', fn)` for runtime method injection. Rust forbids adding methods to types at runtime.

**Solution:** Compile-time `extend_*!` macros that generate `impl` blocks:

```rust
// Third-party package
use ravel::extend_str;

extend_str! {
    fn to_slug(s: &str) -> String {
        s.to_lowercase().replace(' ', "-")
    }
}

// User code
use ravel::facades::Str;
let slug = Str::to_slug("Hello World");  // Compile-time resolution
```

**Layered approach:**

| Scenario | Mechanism | User |
|----------|-----------|------|
| Framework built-ins (`Str::to_slug()`) | Standard `impl` blocks | ravel-crates |
| Third-party package extensions | `extend_str!` macro | Ecosystem developers |
| User project-level extensions | `extend_str!` macro + local definitions | End users |

**Limitations vs Laravel:**
- No runtime registration — must be defined at compile time
- Orphan rule applies — `extend_*!` macros must be provided by the owning crate
- Not dynamic, but zero-cost and type-safe

### 2.4 Application Lifecycle Hooks

**Boot callbacks:**

```rust
Application::new()
    .booting(|app| { Log::info!("Booting..."); })
    .booted(|app| { Log::info!("Ready to serve"); })
    .register_provider(A)
    .boot();
```

**Implementation:** Two `Vec<Box<dyn Fn(&Application)>>` fields on `Application`. Fired in `boot()` before/after provider phases.

### 2.5 Deferrable Service Providers

```rust
pub trait ServiceProvider: Send + Sync {
    // ... existing methods ...

    /// If true, registration is deferred until first resolution
    fn defer(&self) -> bool { false }

    /// TypeIds provided by this provider (for deferred triggering)
    fn provides(&self) -> Vec<TypeId> { vec![] }
}
```

**Mechanism:** Deferred providers register a sentinel in the container. When `resolve::<T>()` encounters a sentinel, it triggers `register()` + `boot()` for that provider, then retries resolution.

**Performance benefit:** CLI commands (`ravel serve` without DB) don't pay connection pool setup cost until first DB access.

---

## Round 3: Ecosystem-Level Completion

**Goal:** Fill missing feature modules so Ravel covers the same scenarios Laravel covers.

### 3.1 Queue Worker Daemon

**CLI:**

```bash
ravel queue:work                    # Default queue
ravel queue:work --queue=mail       # Named queue
ravel queue:work --sleep=3          # Idle sleep seconds
ravel queue:work --tries=3          # Max retry attempts
ravel queue:work --timeout=60       # Job timeout (seconds)
```

**Core loop:**

```rust
pub struct Worker {
    queue: Arc<Queue>,
    sleep: Duration,           // default 3s
    max_tries: u32,
    timeout: Duration,         // default 60s
    shutdown: AtomicBool,
}

impl Worker {
    pub async fn run(&self, queue_name: &str) {
        while !self.shutdown.load(Ordering::SeqCst) {
            match self.queue.pop(queue_name).await {
                Ok(Some(job)) => {
                    self.process_with_timeout(job, queue_name).await;
                }
                Ok(None) => tokio::time::sleep(self.sleep).await,
                Err(e) => { Log::error!("Queue error: {e}"); sleep.await; }
            }
        }
    }
}
```

**Graceful shutdown:**
1. Ctrl+C sets `shutdown = true`
2. Current job finishes (respects timeout)
3. No new jobs accepted
4. Logs "Worker shutting down gracefully"

**Failed job persistence:**

```rust
#[async_trait]
pub trait FailedJobRepository: Send + Sync {
    async fn log(&self, job: &FailedJob) -> Result<()>;
    async fn all(&self) -> Result<Vec<FailedJob>>;
    async fn find(&self, id: &Uuid) -> Result<Option<FailedJob>>;
    async fn forget(&self, id: &Uuid) -> Result<()>;
    async fn flush(&self) -> Result<()>;
}
```

Built-in implementation: `DatabaseFailedJobRepository` (SeaORM-backed table).

### 3.2 Eloquent-Style Model

**`#[derive(Model)]` proc-macro** generates SeaORM boilerplate + Eloquent query API:

```rust
#[derive(Model, Serialize, Deserialize)]
#[model(table = "users")]
struct User {
    #[model(id)]
    id: u32,

    #[model(string, 255)]
    name: String,

    #[model(string, 255, unique)]
    email: String,

    #[model(hidden)]
    #[model(string, 255)]
    password: String,

    #[model(timestamps)]
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}
```

**Generated API:**

```rust
// Querying
let users = User::where("active", true).order_by("created_at", "desc").get().await?;
let user = User::find(1).await?;
let first = User::where("email", "alice@example.com").first().await?;

// CRUD
let user = User::create(json!({"name": "Alice", ...})).await?;
user.update(json!({"email": "new@example.com"})).await?;
user.save().await?;
user.delete().await?;
User::destroy(42).await?;

// Relationships
let posts = user.has_many::<Post>("user_id").get().await?;
let author = post.belongs_to::<User>("user_id").first().await?;

// Pagination
let page = User::where("active", true).paginate(1, 15).await?;

// Eager loading
let users = User::with("posts").get().await?;
```

**`hidden` field handling:** Generates `UserPublic` struct (without hidden fields) + `to_public()` method for JSON responses.

**Design decision:** This wraps SeaORM, not replaces it. The `#[derive(Model)]` macro generates SeaORM's `DeriveEntityModel` + `DeriveActiveModel` + `DeriveColumn` implementations at compile time. Users can still use raw SeaORM for complex queries (subqueries, JOINs). `ModelQuery<T>` is a thin fluent wrapper around `sea_orm::Select<E>`.

**Relationship mapping:** `has_many` and `belongs_to` generate filtered SeaORM queries based on foreign key. The `RelatedModel` trait is auto-derived. Eager loading via `with()` translates to SeaORM's `find_with_related()`.

### 3.3 Blade-Style Template System

**Strategy:** Preprocessor that compiles Blade syntax → Tera templates, then delegates rendering to the existing Tera engine. No new template engine — leverage what's already there.

```
Blade template (.html) → Blade Preprocessor → Tera template (in-memory) → Tera Engine → HTML
```

**Supported directives v1:**

| Blade | Tera output | Notes |
|-------|------------|-------|
| `@extends('layouts.app')` | `{% extends "layouts/app.html" %}` | Layout inheritance |
| `@section('title', 'Text')` | `{% block title %}Text{% endblock %}` | Block with content |
| `@section('title')...@endsection` | `{% block title %}...{% endblock %}` | Multi-line block |
| `@yield('title')` | `{% block title %}{% endblock %}` | Placeholder |
| `@yield('title', 'Default')` | `{% block title %}Default{% endblock %}` | With default |
| `@foreach($users as $user)...@endforeach` | `{% for user in users %}...{% endfor %}` | Strip `$` prefix |
| `@if($active)` / `@else` / `@endif` | `{% if active %}...{% else %}...{% endif %}` | Conditional |
| `@unless($active)...@endunless` | `{% if not active %}...{% endunless %}` | Inverse conditional |
| `@isset($name)...@endisset` | `{% if name is defined %}...{% endif %}` | Variable existence |
| `@empty($items)...@endempty` | `{% if items \| length == 0 %}...{% endif %}` | Empty check |
| `{{ $name }}` | `{{ name }}` | Escaped output |
| `{!! $html !!}` | `{{ html \| safe }}` | Raw HTML |
| `{{ $name ?? 'Default' }}` | `{{ name \| default(value="Default") }}` | Null coalesce |
| `@stack('scripts')` / `@push('scripts')` | Custom context injection | Multi-level push |
| `<x-user-card :user="$user" />` | `{% include "components/user-card.html" %}` | Component with props |

**`@stack` / `@push`:** Collected during preprocessing phase. Parent template's `@stack('name')` is replaced with the concatenation of all `@push('name')` blocks from child templates.

**`<x-component>` syntax:** Converts to `{% include %}` with the component's public attributes passed as context variables.

**Production caching:** In release builds, all templates are compiled once at startup and cached in memory. In debug builds, templates are recompiled on each request (hot-reload).

**Impact on existing code:**
- `view.rs` renamed to `ViewEngine` with pluggable backends (Tera / Blade)
- `Generator` templates remain pure Tera (scaffolding doesn't need Blade)
- Blade preprocessing is lazy — first render triggers compilation

### 3.4 Notification & Mail System

**Multi-channel notifications:**

```rust
#[derive(Notification)]
#[notification(channels = [Mail, Database])]
struct InvoicePaid {
    invoice_id: u32,
    amount: f64,
}

impl NotifyMail for InvoicePaid {
    fn to_mail(&self, notifiable: &impl Notifiable) -> MailMessage {
        MailMessage::new()
            .subject(format!("Invoice #{} Paid", self.invoice_id))
            .line(format!("Amount: ${:.2}", self.amount))
            .action("View Invoice", &format!("/invoices/{}", self.invoice_id))
    }
}

impl NotifyDatabase for InvoicePaid {
    fn to_database(&self, _notifiable: &impl Notifiable) -> DatabaseMessage {
        DatabaseMessage::new(json!({"invoice_id": self.invoice_id, "amount": self.amount}))
    }
}
```

**`Notifiable` trait:**

```rust
#[async_trait]
pub trait Notifiable: Send + Sync {
    fn route_notification_for(&self, channel: &str) -> Option<String>;
    async fn notification_repository(&self) -> Option<Box<dyn DatabaseNotificationRepo>>;
}
```

**Usage:**

```rust
// Send to a user
let user = User::find(1).await?;
user.notify(InvoicePaid { invoice_id: 42, amount: 99.0 }).await?;

// Send to many
let premium_users = User::where("premium", true).get().await?;
Notification::send(&premium_users, InvoicePaid { ... }).await?;

// Route to raw address
Notification::route("mail", "alice@example.com")
    .notify(WelcomeMessage { name: "Alice" })
    .await?;
```

**`mailMessage` fluent builder:**

```rust
MailMessage::new()
    .subject("...")
    .greeting("Hello!")
    .line("First paragraph")
    .line("Second paragraph")
    .action("Click Here", "https://...")
    .salutation("Regards, Team")
```

**Mail drivers:**

```rust
#[async_trait]
pub trait MailDriver: Send + Sync {
    async fn send(&self, to: &str, subject: &str, html: &str, text: &str) -> Result<()>;
}

struct SmtpDriver { config: SmtpConfig }    // Wraps `lettre` crate
struct LogDriver;                            // Logs to tracing for dev
struct ArrayDriver { sent: Mutex<Vec> }       // Testing
```

**Queue integration:** Notifications marked with `queue = "notifications"` are automatically dispatched as jobs instead of sent synchronously.

**Database notifications table:**

| Column | Type | |
|--------|------|---|
| id | u32 (PK) | |
| notifiable_type | String | "User" |
| notifiable_id | u32 | |
| data | JSON | Notification payload |
| read_at | DateTime? | Null = unread |
| created_at | DateTime | |
| updated_at | DateTime | |

---

## What We Don't Do (Intentionally)

| Laravel Feature | Why Not in Rust |
|-----------------|-----------------|
| Runtime reflection for DI | Rust has no runtime reflection — `#[derive(Resolvable)]` macro replaces it |
| `eval()`-style dynamic method registration | Rust `trait` system forbids orphan implementations — `extend_*!` macro is the closest analog |
| Config/Route caching for performance | Rust release build already optimizes to near-zero cost; caching adds complexity without benefit |
| PHP-style `$_SERVER` superglobal | Tokio task-locals provide the same per-request isolation without global mutable state |
| `__call()` / `__get()` magic methods | Rust has no property overloading; macros generate explicit methods instead |
| Dynamic file-based template updates in production | Rust compilation model favors compile-time verification; optional hot-reload in debug mode only |

---

## Implementation Order

1. **Round 1 first** — Facade layer is pure addition, zero breaking changes to existing code. Can ship independently.
2. **Round 2 second** — Pipeline refactors existing middleware but preserves backward compatibility. Auto-DI is additive.
3. **Round 3 last** — New crates (`ravel-eloquent`, `ravel-notification`) and significant extensions to existing ones. Requires Rounds 1-2 as foundation.
