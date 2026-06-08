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
use anyhow::Result;

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
        let repo = ConfigRepo::load_dir(dir)?;
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
    pub fn register_provider<P: ServiceProvider + 'static>(
        mut self,
        provider: P,
    ) -> Self {
        if self.booted {
            // Late registration — register + boot immediately.
            if let Err(e) = provider.register(&self.container) {
                eprintln!(
                    "[ravel] Error registering provider `{}`: {e}",
                    provider.name()
                );
            }
            if let Err(e) = provider.boot(&self.container) {
                eprintln!(
                    "[ravel] Error booting provider `{}`: {e}",
                    provider.name()
                );
            }
        }
        self.providers.push(Box::new(provider));
        self
    }

    /// Bootstrap the application:
    ///
    /// 1. Call `register()` on every provider.
    /// 2. Call `boot()` on every provider.
    /// 3. Register the application itself in the container.
    pub fn boot(mut self) -> Result<Self> {
        if self.booted {
            return Ok(self);
        }

        // Inject self-owned items into the container so providers can
        // resolve them during register().
        self.container.instance_or_replace(self.config.clone());
        self.container.instance_or_replace(self.env.clone());

        // Phase 1 — register
        for provider in &self.providers {
            provider
                .register(&self.container)
                .map_err(|e| {
                    anyhow::anyhow!(
                        "Error registering provider `{}`: {e}",
                        provider.name()
                    )
                })?;
        }

        // Phase 2 — boot
        for provider in &self.providers {
            provider
                .boot(&self.container)
                .map_err(|e| {
                    anyhow::anyhow!(
                        "Error booting provider `{}`: {e}",
                        provider.name()
                    )
                })?;
        }

        self.booted = true;
        Ok(self)
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
            self.call_order
                .fetch_add(1, Ordering::SeqCst);
            c.instance("Registered A".to_string());
            Ok(())
        }

        fn boot(&self, c: &Container) -> Result<()> {
            self.call_order
                .fetch_add(1, Ordering::SeqCst);
            let _val: &String = c.resolve().unwrap();
            Ok(())
        }
    }

    impl ServiceProvider for ProviderB {
        fn register(&self, c: &Container) -> Result<()> {
            self.call_order
                .fetch_add(10, Ordering::SeqCst);
            // Should be able to see what A registered (register runs before boot).
            let _val: &String = c.resolve().unwrap();
            Ok(())
        }

        fn boot(&self, _c: &Container) -> Result<()> {
            self.call_order
                .fetch_add(10, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn test_register_boot_order() {
        let order = Arc::new(AtomicUsize::new(0));

        let app = Application::new()
            .register_provider(ProviderA {
                call_order: order.clone(),
            })
            .register_provider(ProviderB {
                call_order: order.clone(),
            })
            .boot()
            .unwrap();

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

        let repo: &ConfigRepo = app.container.resolve().unwrap();
        assert_eq!(repo.get::<String>("app.name").unwrap(), "RavelTest");
    }

    #[test]
    fn test_env_available_in_container() {
        let dir = std::env::temp_dir().join("ravel_app_env_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut f = std::fs::File::create(dir.join(".env")).unwrap();
        use std::io::Write;
        writeln!(f, "APP_ENV=testing").unwrap();
        drop(f);

        let app = Application::new()
            .load_env(&dir)
            .unwrap()
            .boot()
            .unwrap();

        let env: &EnvRepo = app.container().resolve().unwrap();
        assert_eq!(env.get("APP_ENV"), Some("testing"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_late_provider_registration() {
        let order = Arc::new(AtomicUsize::new(0));

        let app = Application::new()
            .register_provider(ProviderA {
                call_order: order.clone(),
            })
            .boot()
            .unwrap();

        // After boot, registering a new provider immediately calls
        // register+register → the order won't change because ProviderA was already run.
        // But the late provider gets registered and booted immediately.
        let _app = app.register_provider(ProviderB {
            call_order: order.clone(),
        });

        // A.register + A.boot + B.register (late) + B.boot (late) = 1+1+10+10 = 22
        assert_eq!(order.load(Ordering::SeqCst), 22);
    }
}
