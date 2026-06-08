//! Ravel Service Container — a lightweight Dependency Injection container.
//!
//! Inspired by Laravel's service container, it provides:
//! - Binding concrete types (singleton, transient, or pre-built instances)
//! - Lazy resolution via factory closures that receive the container itself
//! - Trait-object bindings via the "key type" idiom
//!
//! Uses interior mutability ([`RefCell`]) so that `resolve` can hand out
//! shared references (`&T`) while still allowing lazy materialisation of
//! singletons behind the scenes.
//!
//! # Concrete-type bindings
//!
//! ```rust
//! use ravel_core::container::Container;
//!
//! #[derive(Debug, PartialEq)]
//! struct AppConfig { name: String }
//!
//! let c = Container::new();
//!
//! // Singleton — resolved once, same reference every time
//! c.singleton(|_| AppConfig { name: "MyApp".into() });
//!
//! let config: &AppConfig = c.resolve().unwrap();
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
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

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
    Instance(Box<dyn Any + Send + Sync>),
}

// ---------------------------------------------------------------------------
// Container
// ---------------------------------------------------------------------------

/// The service container.
///
/// All mutation happens through [`RefCell`] so that `resolve` can take
/// `&self` while still lazily materialising singletons.
pub struct Container {
    entries: RefCell<HashMap<TypeId, Entry>>,
}

impl Container {
    pub fn new() -> Self {
        Self {
            entries: RefCell::new(HashMap::new()),
        }
    }

    // ── Singleton ───────────────────────────────────────────────────

    /// Register a singleton factory.
    ///
    /// # Panics
    /// Panics if a binding for `T` already exists.
    pub fn singleton<T, F>(&self, factory: F)
    where
        T: Send + Sync + 'static,
        F: Fn(&Container) -> T + Send + Sync + 'static,
    {
        let key = TypeId::of::<T>();
        let mut entries = self.entries.borrow_mut();
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
        let f: FactoryFn = Arc::new(move |c| Box::new(factory(c)));
        self.entries
            .borrow_mut()
            .insert(TypeId::of::<T>(), Entry::SingletonFactory(f));
    }

    // ── Instance (pre-built) ────────────────────────────────────────

    /// Register an already-constructed value.
    ///
    /// # Panics
    /// Panics if a binding for `T` already exists.
    pub fn instance<T>(&self, value: T)
    where
        T: Send + Sync + 'static,
    {
        let key = TypeId::of::<T>();
        let mut entries = self.entries.borrow_mut();
        assert!(
            !entries.contains_key(&key),
            "Container: a binding for `{}` already exists",
            std::any::type_name::<T>()
        );
        entries.insert(key, Entry::Instance(Box::new(value)));
    }

    /// Like [`instance`] but overwrites any existing binding.
    pub fn instance_or_replace<T>(&self, value: T)
    where
        T: Send + Sync + 'static,
    {
        self.entries
            .borrow_mut()
            .insert(TypeId::of::<T>(), Entry::Instance(Box::new(value)));
    }

    // ── Transient factory ───────────────────────────────────────────

    /// Register a transient factory — each `resolve_fresh` call creates a
    /// new value.
    ///
    /// # Panics
    /// Panics if a binding for `T` already exists.
    pub fn bind<T, F>(&self, factory: F)
    where
        T: Send + Sync + 'static,
        F: Fn(&Container) -> T + Send + Sync + 'static,
    {
        let key = TypeId::of::<T>();
        let mut entries = self.entries.borrow_mut();
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
        let f: FactoryFn = Arc::new(move |c| Box::new(factory(c)));
        self.entries
            .borrow_mut()
            .insert(TypeId::of::<T>(), Entry::Factory(f));
    }

    // ── Resolution ──────────────────────────────────────────────────

    /// Resolve a singleton or instance binding, returning a shared reference.
    ///
    /// Returns `None` when:
    /// - No binding exists for `T`
    /// - The binding is transient (use [`resolve_fresh`] instead)
    pub fn resolve<T: Send + Sync + 'static>(&self) -> Option<&T> {
        let key = TypeId::of::<T>();

        // Materialise singleton factory if needed.
        let needs_materialise = {
            let entries = self.entries.borrow();
            matches!(
                entries.get(&key),
                None | Some(Entry::SingletonFactory(_))
            )
        };

        if needs_materialise {
            let mut entries = self.entries.borrow_mut();
            if let Some(entry) = entries.remove(&key) {
                match entry {
                    Entry::SingletonFactory(f) => {
                        // Drop borrow before calling factory (it may call back into the container).
                        drop(entries);
                        let instance: Box<dyn Any + Send + Sync> = f(self);
                        self.entries
                            .borrow_mut()
                            .insert(key, Entry::Instance(instance));
                    }
                    Entry::Factory(f) => {
                        entries.insert(key, Entry::Factory(f));
                        return None;
                    }
                    _ => {}
                }
            }
        }

        // Retrieve the (now) Instance.
        // SAFETY: we extend the reference lifetime beyond the Ref-borrow-guard
        // because the Box<dyn Any> lives in the Container, which outlives &self.
        let entries = self.entries.borrow();
        match entries.get(&key) {
            Some(Entry::Instance(boxed)) => {
                let ptr: *const T = boxed.downcast_ref::<T>()?;
                Some(unsafe { &*ptr })
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
            let entries = self.entries.borrow();
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
        self.entries.borrow().contains_key(&TypeId::of::<T>())
    }

    /// Remove the binding for `T`.
    pub fn forget<T: 'static>(&self) {
        self.entries.borrow_mut().remove(&TypeId::of::<T>());
    }

    /// Remove every binding.
    pub fn flush(&self) {
        self.entries.borrow_mut().clear();
    }

    /// Number of registered bindings.
    pub fn len(&self) -> usize {
        self.entries.borrow().len()
    }

    /// `true` when no bindings are registered.
    pub fn is_empty(&self) -> bool {
        self.entries.borrow().is_empty()
    }

    // ── Trait-object API (key-type idiom) ───────────────────────────

    /// Register a trait-object instance keyed by `Key`.
    ///
    /// `Key` is typically an empty unit struct. `Trait` is the object-safe
    /// trait (e.g. `dyn Logger`).  The value is stored as `Arc<Trait>`.
    pub fn bind_trait<Key: 'static, Trait: ?Sized + Send + Sync + 'static>(
        &self,
        value: Arc<Trait>,
    ) {
        self.entries
            .borrow_mut()
            .insert(TypeId::of::<Key>(), Entry::Instance(Box::new(value)));
    }

    /// Resolve a trait-object binding registered via [`bind_trait`].
    pub fn resolve_trait<Key: 'static, Trait: ?Sized + Send + Sync + 'static>(
        &self,
    ) -> Option<Arc<Trait>> {
        self.entries
            .borrow()
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

        let a = c.resolve::<Greeter>().unwrap() as *const Greeter;
        let b = c.resolve::<Greeter>().unwrap() as *const Greeter;
        assert_eq!(a, b);
    }

    #[test]
    fn singleton_factory_receives_container() {
        let c = Container::new();
        c.instance("base:".to_string());
        c.singleton(|cx: &Container| {
            let base: &String = cx.resolve().unwrap();
            Greeter {
                prefix: format!("{base}hello"),
            }
        });

        let g: &Greeter = c.resolve().unwrap();
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
            let base: &u32 = cx.resolve().unwrap();
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
}
