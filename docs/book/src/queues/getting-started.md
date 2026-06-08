# Queues — Getting Started

Queues let you defer time-consuming work (like sending emails, processing uploads,
or calling external APIs) to run outside the HTTP request cycle. Ravel's queue
system provides a `Job` trait, a derive macro, and a `Queue` facade backed by
pluggable drivers.

## The Job Trait

Any struct that implements `Job` represents a unit of work. The trait requires
two methods and provides two optional defaults:

```rust
use ravel_support::queue::Job;
use async_trait::async_trait;

#[async_trait]
impl Job for MyJob {
    /// Execute the job logic.
    async fn handle(&self) -> anyhow::Result<()> { Ok(()) }

    /// Unique name used for routing (required).
    fn name() -> &'static str { "my_job" }

    /// Maximum retry attempts (default: 3).
    fn max_attempts() -> u32 { 3 }

    /// Queue to route this job to (default: "default").
    fn queue() -> &'static str { "default" }
}
```

The `handle()` method receives `&self`, so your job struct carries the data it
needs (e.g. a `user_id`, an email address, a file path).

## Derive Macro

The `#[derive(Job)]` macro generates the trait implementation for you. Use
`#[job(...)]` attributes to configure the job metadata:

```rust
use ravel_macros::Job;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Job)]
#[job(name = "send_welcome", queue = "mail", max_attempts = 5)]
struct SendWelcomeEmail {
    user_id: u32,
    email: String,
}
```

When you omit an attribute the macro falls back to sensible defaults:

- **name** — snake_case of the struct name (e.g. `send_welcome_email`)
- **queue** — `"default"`
- **max_attempts** — `3`

The `#[async_trait]` implementation, the `handle()` method body, and the
`name()` / `queue()` / `max_attempts()` static methods are all generated —
you only need to write `handle()`.

## Job Serialization

Jobs are serialised to JSON with `serde` when dispatched and deserialised when
processed. **Every job struct must derive `Serialize` and `Deserialize`:**

```rust
#[derive(Serialize, Deserialize, Job)]
struct ProcessImage {
    path: String,
    width: u32,
    height: u32,
}
```

## Dispatching Jobs

Use the `Queue` facade to push jobs onto the queue. The application must be
booted with a queue engine registered first:

```rust
use ravel_core::app::Application;
use ravel_facades::ApplicationExt; // provides with_queue()

Application::new()
    .with_queue()                     // registers in-memory queue engine
    .register_provider(MyProvider)
    .boot()?;
```

Then dispatch from anywhere in your application:

```rust
use ravel_facades::Queue;

// Dispatch immediately
Queue::dispatch(SendWelcomeEmail {
    user_id: 42,
    email: "alice@example.com".into(),
})?;

// Dispatch after a delay
Queue::dispatch_later(
    SendWelcomeEmail {
        user_id: 99,
        email: "bob@example.com".into(),
    },
    chrono::Duration::hours(1),
)?;
```

`dispatch_later` accepts any `chrono::Duration`. The job will not be processed
until the delay has elapsed.

## In-Memory Driver

By default, `with_queue()` registers the **in-memory driver** — a
`VecDeque<JobPayload>` behind a `Mutex`. It works out of the box with no
external dependencies. All jobs live in process memory, so they are lost when
the process exits.

For production, swap in a persistent driver (Redis, database) by implementing
the `QueueDriver` trait.

## Example: Welcome Email Job

```rust
use ravel_support::queue::Job;
use ravel_macros::Job;
use ravel_facades::Queue;
use serde::{Serialize, Deserialize};
use async_trait::async_trait;

#[derive(Serialize, Deserialize, Job)]
#[job(name = "send_welcome", queue = "mail", max_attempts = 3)]
struct SendWelcomeEmail {
    user_id: u32,
    email: String,
}

#[async_trait]
impl Job for SendWelcomeEmail {
    async fn handle(&self) -> anyhow::Result<()> {
        // Build and send the email using self.email, self.user_id ...
        tracing::info!("Sending welcome email to {}", self.email);
        Ok(())
    }
}

// In your service provider or route handler:
Queue::dispatch(SendWelcomeEmail {
    user_id: 42,
    email: "alice@example.com".into(),
})?;
```

Next, learn how to process jobs with a [worker daemon](workers.md).
