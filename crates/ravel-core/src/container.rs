//! Ravel Service Container — a lightweight, thread-safe Dependency Injection container.
//!
//! Inspired by Laravel's service container, it provides:
//! - Binding concrete types (singleton, transient, or pre-built instances)
//! - Lazy resolution via factory closures that receive the container itself
//! - Trait-object bindings via the "key type" idiom
//! - A [`freeze()`](Container::freeze) method that locks the container for read-only
//!   access after bootstrap, making it safe for concurrent resolution in Axum.
//!
//! # Concrete-type bindings
//!
//! ```rust
//! use ravel_core::container::Container;
//! use std::sync::Arc;
//!
//! #[derive(Debug, PartialEq)]
//! struct AppConfig { name: String }
//!
//! let c = Container::new();
//!
//! // Singleton — resolved once, same Arc every time
//! c.singleton(|_| AppConfig { name: "MyApp".into() });
//!
//! let config: Arc<AppConfig> = c.resolve().unwrap();
//! assert_eq!(config.name, "MyApp");
//! ```
//!
//! # Trait-object bindings (key-type idiom)
//!
//! ```rust
//! use ravel_core::container::Container;
//! use std::sync::Arc;
//!
//! trait Logger: Send + Sync {
//!     fn log(&self, msg: &str);
//! }
//!
//! struct StdoutLogger;
//! impl Logger for StdoutLogger {
//!     fn log(&self, msg: &str) { println!("{msg}"); }
//! }
//!
//! struct LoggerService;  // key type
//!
//! let c = Container::new();
//! c.bind_trait::<LoggerService, dyn Logger>(Arc::new(StdoutLogger));
//!
//! let logger = c.resolve_trait::<LoggerService, dyn Logger>().unwrap();
//! logger.log("Hello!");
//! ```

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;

// ---------------------------------------------------------------------------
// FactoryFn — type alias for the stored factory callable
// ---------------------------------------------------------------------------

type FactoryFn = Arc<dyn Fn(&Container) -> Box<dyn Any + Send + Sync> + Send + Sync>;

// ---------------------------------------------------------------------------
// Entry
// ---------------------------------------------------------------------------

enum Entry {
    /// A transient factory — each `resolve_fresh` call invokes it.
    Factory(FactoryFn),

    /// A singleton factory — invoked at most once, then cached as `Instance`.
    SingletonFactory(FactoryFn),

    /// A fully materialised value (pre-built or resolved singleton).
    /// Stored as `Box<dyn Any>`; singleton/instance values are wrapped in
    /// `Arc<T>` so that `resolve()` can cheaply clone the pointer.
    Instance(Box<dyn Any + Send + Sync>),
}

// ---------------------------------------------------------------------------
// Container
// ---------------------------------------------------------------------------

/// The service container.
///
/// Uses [`parking_lot::RwLock`] for thread-safe access.  After calling
/// [`freeze()`](Container::freeze), all write operations will panic, but
/// `resolve` can be called concurrently from multiple threads.
pub struct Container {
    entries: RwLock<HashMap<TypeId, Entry>>,
    frozen: AtomicBool,
}

impl Container {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            frozen: AtomicBool::new(false),
        }
    }

    // ── Freeze ──────────────────────────────────────────────────────

    /// Freeze the container, preventing any further writes.
    ///
    /// After freezing, `resolve` can be called safely from multiple threads
    /// (read lock only).  All write methods (`singleton`, `instance`, `bind`,
    /// `bind_trait`, `forget`, `flush`) will **panic** if called after freeze.
    ///
    /// This is called automatically by [`Application::boot()`].
    pub fn freeze(&self) {
        self.frozen.store(true, Ordering::SeqCst);
    }

    /// Check whether the container has been frozen.
    pub fn is_frozen(&self) -> bool {
        self.frozen.load(Ordering::SeqCst)
    }

    /// Panic if the container is frozen (called by write operations).
    fn assert_not_frozen(&self) {
        assert!(
            !self.frozen.load(Ordering::SeqCst),
            "Container is frozen — cannot mutate after boot()"
        );
    }

    // ── Singleton ───────────────────────────────────────────────────

    /// Register a singleton factory.
    ///
    /// # Panics
    /// Panics if a binding for `T` already exists or if the container is frozen.
    pub fn singleton<T, F>(&self, factory: F)
    where
        T: Send + Sync + 'static,
        F: Fn(&Container) -> T + Send + Sync + 'static,
    {
        self.assert_not_frozen();
        let key = TypeId::of::<T>();
        let mut entries = self.entries.write();
        assert!(
            !entries.contains_key(&key),
            "Container: a binding for `{}` already exists",
            std::any::type_name::<T>()
        );
        let f: FactoryFn = Arc::new(move |c| Box::new(factory(c)));
        entries.insert(key, Entry::SingletonFactory(f));
    }

    /// Like [`singleton`] but overwrites any existing binding.
    pub fn singleton_or_replace<T, F>(&self, factory: F)
    where
        T: Send + Sync + 'static,
        F: Fn(&Container) -> T + Send + Sync + 'static,
    {
        self.assert_not_frozen();
        let f: FactoryFn = Arc::new(move |c| Box::new(factory(c)));
        self.entries
            .write()
            .insert(TypeId::of::<T>(), Entry::SingletonFactory(f));
    }

    // ── Instance (pre-built) ────────────────────────────────────────

    /// Register an already-constructed value, wrapped in `Arc<T>` internally.
    ///
    /// # Panics
    /// Panics if a binding for `T` already exists or if the container is frozen.
    pub fn instance<T>(&self, value: T)
    where
        T: Send + Sync + 'static,
    {
        self.assert_not_frozen();
        let key = TypeId::of::<T>();
        let mut entries = self.entries.write();
        assert!(
            !entries.contains_key(&key),
            "Container: a binding for `{}` already exists",
            std::any::type_name::<T>()
        );
        entries.insert(key, Entry::Instance(Box::new(Arc::new(value))));
    }

    /// Like [`instance`] but overwrites any existing binding.
    pub fn instance_or_replace<T>(&self, value: T)
    where
        T: Send + Sync + 'static,
    {
        self.assert_not_frozen();
        self.entries
            .write()
            .insert(TypeId::of::<T>(), Entry::Instance(Box::new(Arc::new(value))));
    }

    // ── Transient factory ───────────────────────────────────────────

    /// Register a transient factory — each `resolve_fresh` call creates a
    /// new value.
    ///
    /// # Panics
    /// Panics if a binding for `T` already exists or if the container is frozen.
    pub fn bind<T, F>(&self, factory: F)
    where
        T: Send + Sync + 'static,
        F: Fn(&Container) -> T + Send + Sync + 'static,
    {
        self.assert_not_frozen();
        let key = TypeId::of::<T>();
        let mut entries = self.entries.write();
        assert!(
            !entries.contains_key(&key),
            "Container: a binding for `{}` already exists",
            std::any::type_name::<T>()
        );
        let f: FactoryFn = Arc::new(move |c| Box::new(factory(c)));
        entries.insert(key, Entry::Factory(f));
    }

    /// Like [`bind`] but overwrites any existing binding.
    pub fn bind_or_replace<T, F>(&self, factory: F)
    where
        T: Send + Sync + 'static,
        F: Fn(&Container) -> T + Send + Sync + 'static,
    {
        self.assert_not_frozen();
        let f: FactoryFn = Arc::new(move |c| Box::new(factory(c)));
        self.entries
            .write()
            .insert(TypeId::of::<T>(), Entry::Factory(f));
    }

    // ── Resolution ──────────────────────────────────────────────────

    /// Resolve a singleton or instance binding, returning an `Arc<T>`.
    ///
    /// The first call on a singleton factory will materialise the value;
    /// subsequent calls return a cheap `Arc::clone`.
    ///
    /// Returns `None` when:
    /// - No binding exists for `T`
    /// - The binding is transient (use [`resolve_fresh`](Self::resolve_fresh) instead)
    pub fn resolve<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        let key = TypeId::of::<T>();

        // Check if we need to materialise a singleton factory.
        let needs_materialise = {
            let entries = self.entries.read();
            matches!(entries.get(&key), Some(Entry::SingletonFactory(_)))
        };

        if needs_materialise {
            // Take the factory out, invoke it, put the Instance back.
            let factory = {
                let mut entries = self.entries.write();
                match entries.remove(&key)? {
                    Entry::SingletonFactory(f) => f,
                    other => {
                        entries.insert(key, other);
                        // Fall through to read below
                        return self.read_instance::<T>();
                    }
                }
            };

            // Invoke factory WITHOUT holding any lock (it may call back into the container).
            let raw: Box<dyn Any + Send + Sync> = factory(self);

            // Unpack the raw value as T, wrap in Arc<T>, and store.
            let mut entries = self.entries.write();
            match raw.downcast::<T>() {
                Ok(typed) => {
                    let arc: Arc<T> = Arc::from(*typed);
                    entries.insert(key, Entry::Instance(Box::new(arc)));
                }
                Err(_) => return None, // type mismatch — should not happen
            }
        }

        self.read_instance::<T>()
    }

    /// Read an already-materialised Instance from the map, returning `Arc<T>`.
    fn read_instance<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        let entries = self.entries.read();
        match entries.get(&TypeId::of::<T>())? {
            Entry::Instance(boxed) => {
                let arc_ref: &Arc<T> = boxed.downcast_ref::<Arc<T>>()?;
                Some(Arc::clone(arc_ref))
            }
            _ => None,
        }
    }

    /// Resolve a transient factory binding, returning an **owned** `T`.
    /// Each call invokes the factory anew.
    ///
    /// Returns `None` when no transient binding exists for `T`.
    pub fn resolve_fresh<T: Send + Sync + 'static>(&self) -> Option<T> {
        let key = TypeId::of::<T>();

        // Clone the Arc so we can call the factory without holding a borrow.
        let f: FactoryFn = {
            let entries = self.entries.read();
            match entries.get(&key)? {
                Entry::Factory(f) => Arc::clone(f),
                _ => return None,
            }
        };

        let instance = f(self);
        instance.downcast::<T>().ok().map(|b| *b)
    }

    // ── Introspection ───────────────────────────────────────────────

    /// Check whether any binding exists for `T`.
    pub fn has<T: 'static>(&self) -> bool {
        self.entries.read().contains_key(&TypeId::of::<T>())
    }

    /// Remove the binding for `T`.
    ///
    /// # Panics
    /// Panics if the container is frozen.
    pub fn forget<T: 'static>(&self) {
        self.assert_not_frozen();
        self.entries.write().remove(&TypeId::of::<T>());
    }

    /// Remove every binding.
    ///
    /// # Panics
    /// Panics if the container is frozen.
    pub fn flush(&self) {
        self.assert_not_frozen();
        self.entries.write().clear();
    }

    /// Number of registered bindings.
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// `true` when no bindings are registered.
    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }

    // ── Trait-object API (key-type idiom) ───────────────────────────

    /// Register a trait-object instance keyed by `Key`.
    ///
    /// `Key` is typically an empty unit struct. `Trait` is the object-safe
    /// trait (e.g. `dyn Logger`).  The value is stored as `Arc<Trait>`.
    ///
    /// # Panics
    /// Panics if the container is frozen.
    pub fn bind_trait<Key: 'static, Trait: ?Sized + Send + Sync + 'static>(
        &self,
        value: Arc<Trait>,
    ) {
        self.assert_not_frozen();
        self.entries
            .write()
            .insert(TypeId::of::<Key>(), Entry::Instance(Box::new(value)));
    }

    /// Resolve a trait-object binding registered via [`bind_trait`].
    pub fn resolve_trait<Key: 'static, Trait: ?Sized + Send + Sync + 'static>(
        &self,
    ) -> Option<Arc<Trait>> {
        self.entries
            .read()
            .get(&TypeId::of::<Key>())
            .and_then(|entry| match entry {
                Entry::Instance(boxed) => boxed.downcast_ref::<Arc<Trait>>().cloned(),
                _ => None,
            })
    }
}

impl Default for Container {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Debug, PartialEq)]
    struct Greeter {
        prefix: String,
    }

    // ── Singleton ─────────────────────────────────────────────────

    #[test]
    fn singleton_returns_same_instance() {
        let c = Container::new();
        c.singleton(|_| Greeter {
            prefix: "Hi".into(),
        });

        let a = c.resolve::<Greeter>().unwrap();
        let b = c.resolve::<Greeter>().unwrap();
        // Same Arc → same pointer
        assert_eq!(Arc::as_ptr(&a), Arc::as_ptr(&b));
    }

    #[test]
    fn singleton_factory_receives_container() {
        let c = Container::new();
        c.instance("base:".to_string());
        c.singleton(|cx: &Container| {
            let base: Arc<String> = cx.resolve().unwrap();
            Greeter {
                prefix: format!("{base}hello"),
            }
        });

        let g = c.resolve::<Greeter>().unwrap();
        assert_eq!(g.prefix, "base:hello");
    }

    // ── Instance ──────────────────────────────────────────────────

    #[test]
    fn prebuilt_instance() {
        let c = Container::new();
        c.instance(42u32);
        assert_eq!(*c.resolve::<u32>().unwrap(), 42);
    }

    // ── Transient factory ─────────────────────────────────────────

    #[test]
    fn transient_factory_new_each_time() {
        let c = Container::new();
        let counter = Arc::new(Mutex::new(0u32));
        let cnt = counter.clone();
        c.bind(move |_| {
            let mut n = cnt.lock().unwrap();
            *n += 1;
            *n
        });

        assert_eq!(c.resolve_fresh::<u32>().unwrap(), 1);
        assert_eq!(c.resolve_fresh::<u32>().unwrap(), 2);
        assert_eq!(c.resolve_fresh::<u32>().unwrap(), 3);

        // resolve() returns None for transient factories
        assert!(c.resolve::<u32>().is_none());
    }

    #[test]
    fn transient_factory_receives_container() {
        let c = Container::new();

        // Register a base value that the transient factory reads
        c.instance(100u32);

        c.bind(move |cx: &Container| {
            let base: Arc<u32> = cx.resolve().unwrap();
            format!("value={base}")
        });

        assert_eq!(c.resolve_fresh::<String>().unwrap(), "value=100");
    }

    // ── Trait objects ─────────────────────────────────────────────

    #[test]
    fn trait_object_binding() {
        trait Calc: Send + Sync {
            fn double(&self, x: i32) -> i32;
        }

        struct Doubler;
        impl Calc for Doubler {
            fn double(&self, x: i32) -> i32 {
                x * 2
            }
        }

        struct CalcService;

        let c = Container::new();
        c.bind_trait::<CalcService, dyn Calc>(Arc::new(Doubler));

        let calc = c.resolve_trait::<CalcService, dyn Calc>().unwrap();
        assert_eq!(calc.double(5), 10);

        // Same Arc
        let calc2 = c.resolve_trait::<CalcService, dyn Calc>().unwrap();
        assert_eq!(Arc::as_ptr(&calc), Arc::as_ptr(&calc2));
    }

    // ── Introspection ─────────────────────────────────────────────

    #[test]
    fn has_and_forget() {
        let c = Container::new();
        assert!(!c.has::<u32>());

        c.instance(1u32);
        assert!(c.has::<u32>());

        c.forget::<u32>();
        assert!(!c.has::<u32>());
    }

    #[test]
    fn flush_removes_all() {
        let c = Container::new();
        c.instance(1u32);
        c.instance("hello");
        assert_eq!(c.len(), 2);

        c.flush();
        assert!(c.is_empty());
    }

    // ── Freeze ────────────────────────────────────────────────────

    #[test]
    fn freeze_prevents_writes() {
        let c = Container::new();
        c.instance(42u32);
        c.freeze();

        assert!(c.is_frozen());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            c.instance(99u32);
        }));
        assert!(result.is_err());
    }

    #[test]
    fn resolve_works_after_freeze() {
        let c = Container::new();
        c.singleton(|_| Greeter { prefix: "Hi".into() });
        c.freeze();

        // Should still be able to resolve after freeze
        let g = c.resolve::<Greeter>().unwrap();
        assert_eq!(g.prefix, "Hi");
    }

    #[test]
    fn concurrent_resolve_after_freeze() {
        use std::thread;

        let c = Arc::new(Container::new());
        c.singleton(|_| Greeter { prefix: "Threaded".into() });
        c.freeze();

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let c = Arc::clone(&c);
                thread::spawn(move || {
                    let g = c.resolve::<Greeter>().unwrap();
                    assert_eq!(g.prefix, "Threaded");
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }
    }
}
