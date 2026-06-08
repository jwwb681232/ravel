//! Event system — dispatch events and notify registered listeners.
//!
//! Inspired by Laravel's event system, this module provides:
//! - `Event` trait — marker for any event payload
//! - `Listener<E>` trait — handles a specific event type
//! - `EventDispatcher` — registry + dispatch
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_core::events::{Event, Listener, EventDispatcher};
//! use std::sync::Arc;
//!
//! #[derive(Clone)]
//! struct UserCreated { name: String }
//! impl Event for UserCreated {}
//!
//! struct SendWelcomeEmail;
//! impl Listener<UserCreated> for SendWelcomeEmail {
//!     fn handle(&self, event: &UserCreated) {
//!         println!("Sending welcome email to {}", event.name);
//!     }
//! }
//!
//! let mut dispatcher = EventDispatcher::new();
//! dispatcher.listen(UserCreated { name: "Alice".into() }, Arc::new(SendWelcomeEmail));
//! dispatcher.dispatch(&UserCreated { name: "Bob".into() });
//! ```

use std::collections::HashMap;
use std::any::TypeId;
use std::sync::Arc;

/// Marker trait for event payloads.  
/// Events should be `Clone + Send + Sync` so they can be dispatched freely.
pub trait Event: Clone + Send + Sync + 'static {}

/// A listener that handles events of type `E`.
pub trait Listener<E: Event>: Send + Sync {
    /// Handle the event.
    fn handle(&self, event: &E);
}

/// Holds listener registrations and dispatches events.
pub struct EventDispatcher {
    listeners: HashMap<TypeId, Vec<Arc<ErasedListener>>>,
}

/// Type-erased listener — wraps a closure so we don't need generics here.
struct ErasedListener {
    handle: Box<dyn Fn(&(dyn std::any::Any + Send + Sync)) + Send + Sync>,
}

impl ErasedListener {
    fn new<E: Event, L: Listener<E> + 'static>(listener: Arc<L>) -> Self {
        // Move the Arc into the closure so it owns the listener
        let l = listener;
        ErasedListener {
            handle: Box::new(move |event: &(dyn std::any::Any + Send + Sync)| {
                if let Some(e) = event.downcast_ref::<E>() {
                    l.handle(e);
                }
            }),
        }
    }

    fn handle_any(&self, event: &(dyn std::any::Any + Send + Sync)) {
        (self.handle)(event);
    }
}

impl EventDispatcher {
    pub fn new() -> Self {
        Self {
            listeners: HashMap::new(),
        }
    }

    /// Register a listener for event type `E`.
    ///
    /// ```rust,ignore
    /// dispatcher.listen(UserCreated { .. }, Arc::new(SendWelcomeEmail));
    /// ```
    /// The first argument is only used for type inference — its value is ignored.
    pub fn listen<E: Event, L: Listener<E> + 'static>(&mut self, _dummy: E, listener: Arc<L>) {
        self.listeners
            .entry(TypeId::of::<E>())
            .or_default()
            .push(Arc::new(ErasedListener::new::<E, L>(listener)));
    }

    /// Dispatch an event to all registered listeners.
    pub fn dispatch<E: Event>(&self, event: &E) {
        if let Some(listeners) = self.listeners.get(&TypeId::of::<E>()) {
            for listener in listeners {
                listener.handle_any(event);
            }
        }
    }

    /// Remove all listeners for event type `E`.
    pub fn forget<E: Event>(&mut self) {
        self.listeners.remove(&TypeId::of::<E>());
    }

    /// Remove all listeners.
    pub fn flush(&mut self) {
        self.listeners.clear();
    }

    /// Number of registered event types.
    pub fn len(&self) -> usize {
        self.listeners.len()
    }

    pub fn is_empty(&self) -> bool {
        self.listeners.is_empty()
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone)]
    struct OrderPlaced {
        order_id: u32,
    }
    impl Event for OrderPlaced {}

    struct OrderLogger {
        counter: Arc<AtomicUsize>,
    }
    impl Listener<OrderPlaced> for OrderLogger {
        fn handle(&self, e: &OrderPlaced) {
            self.counter.fetch_add(e.order_id as usize, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_dispatch_to_single_listener() {
        let counter = Arc::new(AtomicUsize::new(0));
        let logger = OrderLogger { counter: counter.clone() };

        let mut d = EventDispatcher::new();
        d.listen(OrderPlaced { order_id: 0 }, Arc::new(logger));
        d.dispatch(&OrderPlaced { order_id: 42 });

        assert_eq!(counter.load(Ordering::SeqCst), 42);
    }

    #[test]
    fn test_multiple_listeners() {
        let counter = Arc::new(AtomicUsize::new(0));

        let mut d = EventDispatcher::new();
        d.listen(OrderPlaced { order_id: 0 }, Arc::new(OrderLogger { counter: counter.clone() }));
        d.listen(OrderPlaced { order_id: 0 }, Arc::new(OrderLogger { counter: counter.clone() }));

        d.dispatch(&OrderPlaced { order_id: 10 }); // 10 + 10 = 20

        assert_eq!(counter.load(Ordering::SeqCst), 20);
    }

    #[test]
    fn test_forget_removes_listeners() {
        let counter = Arc::new(AtomicUsize::new(0));
        let mut d = EventDispatcher::new();
        d.listen(OrderPlaced { order_id: 0 }, Arc::new(OrderLogger { counter: counter.clone() }));
        d.forget::<OrderPlaced>();
        d.dispatch(&OrderPlaced { order_id: 100 });
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }
}
