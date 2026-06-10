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

The `#[derive(Job)]` macro generates the entire `Job` trait implementation. Use `#[job(...)]` attributes to configure metadata:

```rust
use ravel_macros::Job;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Job)]
#[job(name = "send_welcome", queue = "mail", max_attempts = 5)]
struct SendWelcomeEmail {
    user_id: u32,
    email: String,
}

impl SendWelcomeEmail {
    /// Core logic — called automatically by the generated handle().
    pub async fn execute(&self) -> anyhow::Result<()> {
        tracing::info!("Sending welcome email to {}", self.email);
        Ok(())
    }
}
```

When you omit attributes the macro falls back to sensible defaults:

- **name** — snake_case of the struct name (e.g. `send_welcome_email`)
- **queue** — `"default"`
- **max_attempts** — `3`

> **Note:** Do **not** manually `impl Job` when using `#[derive(Job)]` — the macro already generates it. Write an inherent `pub async fn execute(&self)` method instead.

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

## Redis Driver

For production, use the **Redis driver** to persist jobs across process restarts and enable multiple workers. Enable the `redis` feature:

```toml
ravel-support = { features = ["redis"] }
```

```rust
use ravel_support::queue::{Queue, RedisDriver};

// Connect and create queue
let queue = Queue::redis("redis://127.0.0.1:6379").await?;
queue.register::<SendWelcomeEmail>();
queue.dispatch(SendWelcomeEmail { ... }).await?;
queue.run().await;
```

How it works:

- Active jobs are stored in a Redis **LIST** (`RPUSH` / `LPOP` — atomic)
- Delayed jobs are stored in a **ZSET** (score = unix timestamp)
- On `pop()`, ready delayed jobs automatically migrate to the main LIST
- Multiple workers can safely consume from the same Redis instance

### Redis Key Format

```
ravel:queue:{name}          →  LIST   — active jobs
ravel:queue:{name}:delayed  →  ZSET   — delayed jobs (score = Unix ms)
```

## In-Memory Driver (Development)

## Example: Welcome Email Job

```rust
use ravel_macros::Job;
use ravel_facades::Queue;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Job)]
#[job(name = "send_welcome", queue = "mail", max_attempts = 3)]
struct SendWelcomeEmail {
    user_id: u32,
    email: String,
}

impl SendWelcomeEmail {
    pub async fn execute(&self) -> anyhow::Result<()> {
        tracing::info!("Sending welcome email to {}", self.email);
        Ok(())
    }
}

// Dispatch:
Queue::dispatch(SendWelcomeEmail {
    user_id: 42,
    email: "alice@example.com".into(),
})?;
```

Next, learn how to process jobs with a [worker daemon](workers.md).
