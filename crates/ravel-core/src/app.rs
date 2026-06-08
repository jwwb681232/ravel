//! ServiceProvider mechanism and Application lifecycle.
//!
//! Inspired by Laravel, every Ravel application starts with an
//! [`Application`] that:
//!
//! 1. Loads configuration from the `config/` directory.
//! 2. Registers service providers (call `register()` on each).
//! 3. Boots all providers (call `boot()` on each).
//!
//! # Writing a provider
//!
//! ```rust,ignore
//! use ravel_core::{app::ServiceProvider, container::Container};
//!
//! struct RouteServiceProvider;
//!
//! impl ServiceProvider for RouteServiceProvider {
//!     fn register(&self, app: &Container) {
//!         // Bind route-related services into the container.
//!     }
//!
//!     fn boot(&self, app: &Container) {
//!         // Load routes — all providers are registered at this point.
//!     }
//! }
//! ```

use crate::config::ConfigRepo;
use crate::container::Container;
use crate::env::EnvRepo;
use crate::log::{self, Log};
use anyhow::Result;
use std::ops::Deref;
use std::sync::{Mutex, MutexGuard};

/// Global application instance, set by [`Application::boot()`].
/// All facades read from this singleton.
pub static APP: AppGlobal = AppGlobal::new();

/// Thread-safe wrapper around the global [`Application`] singleton.
///
/// In production, [`Application::boot()`] stores the app here once.
/// For testing, [`AppGlobal::reset`] is available to clear the value between
/// test cases.
pub struct AppGlobal {
    inner: Mutex<Option<Application>>,
}

impl AppGlobal {
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    /// Store the application as the global singleton.
    ///
    /// Returns `Err(app)` if the global has already been set.
    pub fn set(&self, app: Application) -> Result<(), Application> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_some() {
            return Err(app);
        }
        *guard = Some(app);
        Ok(())
    }

    /// Obtain a reference to the global application.
    ///
    /// Returns `None` if [`Application::boot()`] has not been called yet.
    pub fn get(&self) -> Option<AppRef<'_>> {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_some() {
            Some(AppRef(guard))
        } else {
            None
        }
    }
}

impl AppGlobal {
    /// Reset the global app so the next call to `boot()` succeeds.
    ///
    /// In production this is rarely needed; it is primarily used between
    /// test cases that share the same process.
    ///
    /// Recovers from a poisoned mutex (which can happen after a
    /// `#[should_panic]` test unwinds while holding the APP lock) so that
    /// subsequent test cases can continue without a panic cascade.
    pub fn reset(&self) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        *guard = None;
    }
}

/// Serialise boot-related tests so they don't race on the global `APP`.
#[cfg(test)]
pub(crate) static BOOT_LOCK: Mutex<()> = Mutex::new(());

/// A locked reference to the global [`Application`].
///
/// Holds the internal lock so the application is not modified while being
/// accessed. Created by [`AppGlobal::get`].
pub struct AppRef<'a>(MutexGuard<'a, Option<Application>>);

impl<'a> Deref for AppRef<'a> {
    type Target = Application;

    fn deref(&self) -> &Application {
        // Safety: `AppRef` is only constructed when the inner `Option` is
        // `Some`, so unwrapping is always safe.
        self.0.as_ref().unwrap()
    }
}

// ── ServiceProvider trait ──────────────────────────────────────────────

/// A service provider that adds functionality to the application.
///
/// Implementations should:
/// - **In `register()`**: bind things into the container.
///   All providers' `register()` methods are called before any `boot()`.
/// - **In `boot()`**: perform startup logic that depends on every provider
///   already being registered (e.g. loading route files, starting listeners).
///
/// Both methods receive `&Container` — use `container.singleton()`,
/// `container.instance()`, etc. to register services.
pub trait ServiceProvider: Send + Sync {
    /// Called during application bootstrap, before any provider is booted.
    /// Use this to bind services into the container.
    fn register(&self, container: &Container) -> Result<()>;

    /// Called after all providers have been registered.
    /// Use this for wiring that requires other services to already exist.
    fn boot(&self, container: &Container) -> Result<()> {
        let _ = container;
        Ok(()) // default: no-op
    }

    /// Optional name for debugging / display.
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
}

// ── Application ────────────────────────────────────────────────────────

/// The Ravel application kernel.
///
/// Owns the service container, config repository, and the list of providers.
///
/// # Quick start
///
/// ```rust,ignore
/// use ravel_core::app::Application;
///
/// let app = Application::new()
///     .load_config("config")
///     .unwrap()
///     .register_provider(my_provider)
///     .boot()
///     .unwrap();
///
/// // Access resolved services via the container:
/// let db: &Database = app.container().resolve().unwrap();
/// ```
pub struct Application {
    container: Container,
    config: ConfigRepo,
    env: EnvRepo,
    providers: Vec<Box<dyn ServiceProvider>>,
    booted: bool,
}

impl Application {
    /// Create a new, empty application.
    pub fn new() -> Self {
        let container = Container::new();
        Self {
            container,
            config: ConfigRepo::new(),
            env: EnvRepo::new(),
            providers: Vec::new(),
            booted: false,
        }
    }

    /// Load TOML configuration from `dir` and store it both in the
    /// internal [`ConfigRepo`] and in the container (as a singleton).
    pub fn load_config(mut self, dir: impl AsRef<std::path::Path>) -> Result<Self> {
        let mut repo = ConfigRepo::load_dir(dir)?;
        repo.apply_env_overrides();
        self.config = repo.clone();
        self.container.instance(repo);
        Ok(self)
    }

    /// Load a `.env` file and store it as an [`EnvRepo`] singleton.
    ///
    /// If `path` is a directory, looks for `.env` inside it.
    pub fn load_env(mut self, path: impl AsRef<std::path::Path>) -> Result<Self> {
        let path = path.as_ref();
        let repo = if path.is_dir() {
            EnvRepo::load_from_dir(path)?
        } else {
            EnvRepo::load(path)?
        };
        self.env = repo.clone();
        self.container.instance_or_replace(repo);
        Ok(self)
    }

    /// Register a service provider.
    ///
    /// If the application has already been booted the provider is
    /// registered **and** booted immediately.
    pub fn register_provider<P: ServiceProvider + 'static>(mut self, provider: P) -> Self {
        if self.booted {
            // Late registration — register + boot immediately.
            if let Err(e) = provider.register(&self.container) {
                log::error!("Error registering provider `{}`: {e}", provider.name());
            }
            if let Err(e) = provider.boot(&self.container) {
                log::error!("Error booting provider `{}`: {e}", provider.name());
            }
        }
        self.providers.push(Box::new(provider));
        self
    }

    /// Bootstrap the application:
    ///
    /// 1. Initialise structured logging.
    /// 2. Call `register()` on every provider.
    /// 3. Call `boot()` on every provider.
    /// 4. Register the application itself in the container.
    pub fn boot(mut self) -> Result<()> {
        if self.booted {
            return Ok(());
        }

        // Initialise logging (respects RAVEL_LOG env var, defaults to "info")
        Log::init("info");

        // Inject self-owned items into the container so providers can
        // resolve them during register().
        self.container.instance_or_replace(self.config.clone());
        self.container.instance_or_replace(self.env.clone());

        // Phase 1 — register
        for provider in &self.providers {
            provider.register(&self.container).map_err(|e| {
                anyhow::anyhow!("Error registering provider `{}`: {e}", provider.name())
            })?;
        }

        // Phase 2 — boot
        for provider in &self.providers {
            provider.boot(&self.container).map_err(|e| {
                anyhow::anyhow!("Error booting provider `{}`: {e}", provider.name())
            })?;
        }

        self.booted = true;

        // Freeze the container for thread-safe reads
        self.container.freeze();

        // Store as global singleton for facades
        APP.set(self)
            .map_err(|_| anyhow::anyhow!("Application already booted"))?;

        log::info!("Application booted successfully");

        Ok(())
    }

    /// Return a reference to the service container.
    pub fn container(&self) -> &Container {
        &self.container
    }

    /// Return a reference to the config repository.
    pub fn config(&self) -> &ConfigRepo {
        &self.config
    }

    /// Return a reference to the environment repository.
    pub fn env(&self) -> &EnvRepo {
        &self.env
    }

    /// Check whether the application has been booted.
    pub fn is_booted(&self) -> bool {
        self.booted
    }

    /// Register a provider after boot (late registration).
    ///
    /// The provider's `register()` and `boot()` MUST NOT write to the container
    /// (container is frozen). Only for providers that read existing services.
    pub fn register_provider_post_boot<P: ServiceProvider + 'static>(&self, provider: P) {
        if let Err(e) = provider.register(&self.container) {
            log::error!("Error registering provider `{}`: {e}", provider.name());
        }
        if let Err(e) = provider.boot(&self.container) {
            log::error!("Error booting provider `{}`: {e}", provider.name());
        }
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}

// ── Convenience: register plain services ───────────────────────────────

impl Application {
    /// Register a singleton factory (shorthand for `self.container.singleton(f)`).
    pub fn singleton<T, F>(&self, factory: F)
    where
        T: Send + Sync + 'static,
        F: Fn(&Container) -> T + Send + Sync + 'static,
    {
        self.container.singleton(factory);
    }

    /// Register a pre-built instance (shorthand for `self.container.instance(v)`).
    pub fn instance<T>(&self, value: T)
    where
        T: Send + Sync + 'static,
    {
        self.container.instance(value);
    }

    /// Register the in-memory cache so facades can use `Cache::put()`, `Cache::get()` etc.
    pub fn with_cache(self) -> Self {
        self.container.instance(crate::cache::MemoryCache::new());
        self
    }

    /// Register the encrypter from an APP_KEY string.
    /// Accepts the `base64:` prefix format produced by `ravel key:generate`.
    pub fn with_app_key(self, key: &str) -> Result<Self> {
        let crypt = crate::crypt::Crypt::from_key(key)?;
        self.container.instance(crypt);
        Ok(self)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct ProviderA {
        call_order: Arc<AtomicUsize>,
    }
    struct ProviderB {
        call_order: Arc<AtomicUsize>,
    }

    impl ServiceProvider for ProviderA {
        fn register(&self, c: &Container) -> Result<()> {
            self.call_order.fetch_add(1, Ordering::SeqCst);
            c.instance("Registered A".to_string());
            Ok(())
        }

        fn boot(&self, c: &Container) -> Result<()> {
            self.call_order.fetch_add(1, Ordering::SeqCst);
            let _val: std::sync::Arc<String> = c.resolve().unwrap();
            Ok(())
        }
    }

    impl ServiceProvider for ProviderB {
        fn register(&self, c: &Container) -> Result<()> {
            self.call_order.fetch_add(10, Ordering::SeqCst);
            // Should be able to see what A registered (register runs before boot).
            let _val: std::sync::Arc<String> = c.resolve().unwrap();
            Ok(())
        }

        fn boot(&self, _c: &Container) -> Result<()> {
            self.call_order.fetch_add(10, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn test_register_boot_order() {
        let _lock = BOOT_LOCK.lock().unwrap();
        APP.reset();

        let order = Arc::new(AtomicUsize::new(0));

        Application::new()
            .register_provider(ProviderA {
                call_order: order.clone(),
            })
            .register_provider(ProviderB {
                call_order: order.clone(),
            })
            .boot()
            .unwrap();

        let app = APP.get().unwrap();
        assert!(app.is_booted());

        // A.register = +1, B.register = +10, A.boot = +1, B.boot = +10 → 22
        assert_eq!(order.load(Ordering::SeqCst), 22);
    }

    #[test]
    fn test_config_available_in_container() {
        let mut app = Application::new();
        app.config.set("app.name", "RavelTest");

        // Put it in the container manually (normally done by load_config)
        app.container.instance(app.config.clone());

        let repo: std::sync::Arc<ConfigRepo> = app.container.resolve().unwrap();
        assert_eq!(repo.get::<String>("app.name").unwrap(), "RavelTest");
    }

    #[test]
    fn test_env_available_in_container() {
        let _lock = BOOT_LOCK.lock().unwrap();
        APP.reset();

        let dir = std::env::temp_dir().join("ravel_app_env_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut f = std::fs::File::create(dir.join(".env")).unwrap();
        use std::io::Write;
        writeln!(f, "APP_ENV=testing").unwrap();
        drop(f);

        Application::new().load_env(&dir).unwrap().boot().unwrap();

        let app = APP.get().unwrap();
        let env: std::sync::Arc<EnvRepo> = app.container().resolve().unwrap();
        assert_eq!(env.get("APP_ENV"), Some("testing"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_late_provider_registration() {
        let _lock = BOOT_LOCK.lock().unwrap();
        APP.reset();

        let order = Arc::new(AtomicUsize::new(0));

        Application::new()
            .register_provider(ProviderA {
                call_order: order.clone(),
            })
            .boot()
            .unwrap();

        // Late registration via post-boot API (reads from frozen container only)
        let app = APP.get().unwrap();
        app.register_provider_post_boot(ProviderB {
            call_order: order.clone(),
        });

        // A.register + A.boot + B.register (late) + B.boot (late) = 1+1+10+10 = 22
        assert_eq!(order.load(Ordering::SeqCst), 22);
    }
}
