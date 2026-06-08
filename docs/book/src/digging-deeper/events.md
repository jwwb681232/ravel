# Events

Ravel's event system provides a lightweight publish/subscribe mechanism. You define events
as plain structs and attach listeners that react to them. Multiple listeners can observe the
same event, and listeners are fully decoupled from the dispatch site.

## Event and Listener Traits

Any type that implements `Clone + Send + Sync + 'static` can be an event by implementing the
`Event` marker trait. Listeners implement `Listener<E>` for the specific event they handle.

```rust
use ravel_core::events::{Event, Listener, EventDispatcher};
use std::sync::Arc;

#[derive(Clone)]
struct UserCreated {
    user_id: u32,
    email: String,
}

impl Event for UserCreated {}

struct SendWelcomeEmail;

impl Listener<UserCreated> for SendWelcomeEmail {
    fn handle(&self, event: &UserCreated) {
        println!("Sending welcome email to {}", event.email);
    }
}

struct UpdateAnalytics;

impl Listener<UserCreated> for UpdateAnalytics {
    fn handle(&self, event: &UserCreated) {
        println!("Recording signup for user {}", event.user_id);
    }
}
```

## Dispatching Events

Create an `EventDispatcher`, register listeners, and dispatch.

```rust
let mut dispatcher = EventDispatcher::new();

// Register two listeners for the same event
dispatcher.listen(
    UserCreated { user_id: 0, email: String::new() }, // dummy for type inference
    Arc::new(SendWelcomeEmail),
);
dispatcher.listen(
    UserCreated { user_id: 0, email: String::new() },
    Arc::new(UpdateAnalytics),
);

// Dispatch — both listeners fire in registration order
dispatcher.dispatch(&UserCreated {
    user_id: 42,
    email: "alice@example.com".into(),
});
```

## Removing Listeners

Use `forget` to remove all listeners for an event type, or `flush` to clear everything.

```rust
dispatcher.forget::<UserCreated>();  // Remove all UserCreated listeners
dispatcher.flush();                  // Remove every listener
```

## Example: User Registration Flow

```rust
use ravel_core::events::{Event, Listener, EventDispatcher};
use std::sync::Arc;

// Define the event payload
#[derive(Clone)]
struct UserRegistered {
    user_id: u32,
    name: String,
    email: String,
}

impl Event for UserRegistered {}

// Listener 1: send a welcome email
struct EmailListener;

impl Listener<UserRegistered> for EmailListener {
    fn handle(&self, event: &UserRegistered) {
        // SendEmail::to(&event.email).subject("Welcome!").send();
        println!("Email sent to {}", event.email);
    }
}

// Listener 2: log analytics
struct AnalyticsListener;

impl Listener<UserRegistered> for AnalyticsListener {
    fn handle(&self, event: &UserRegistered) {
        println!("Analytics: user {} ({}) registered", event.user_id, event.name);
    }
}

// Wire them up
let mut events = EventDispatcher::new();
events.listen(UserRegistered { user_id: 0, name: String::new(), email: String::new() }, Arc::new(EmailListener));
events.listen(UserRegistered { user_id: 0, name: String::new(), email: String::new() }, Arc::new(AnalyticsListener));

// Dispatch when a user signs up
async fn register_user(name: String, email: String, events: &EventDispatcher) {
    let user_id = create_user(&name, &email).await;
    events.dispatch(&UserRegistered { user_id, name, email });
}
```

## API Reference

| Method | Description |
|--------|-------------|
| `EventDispatcher::new()` | Create a new dispatcher |
| `listen(dummy, listener)` | Register a `Listener<E>` using a dummy event for type inference |
| `dispatch(event)` | Dispatch an event to all registered listeners |
| `forget::<E>()` | Remove all listeners for event type `E` |
| `flush()` | Remove all listeners for all event types |

## Design Notes

- Listeners are stored as type-erased `Arc` pointers, so different event types can coexist
  in the same dispatcher without generic machinery.
- The first argument to `listen` is a dummy value used only for type inference; its data is
  never inspected.
- The `Event` trait is intentionally a marker — it enforces `Clone + Send + Sync + 'static`
  without dictating any methods.
