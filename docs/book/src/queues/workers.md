# Workers

A **worker** is a long-running process that pops jobs from the queue and
executes them. Ravel provides a built-in daemon command and a
programmatic API for processing jobs.

## The Worker Daemon

Start the worker with the CLI command:

```bash
ravel queue:work
```

This boots the application and enters a loop that polls the queue for pending
jobs. When a job is found, it is deserialised, its `handle()` method is called,
and the result is acknowledged or retried.

### Options

| Flag              | Default    | Description                              |
|-------------------|------------|------------------------------------------|
| `--queue`         | `"default"`| Queue name to consume from               |
| `--sleep`         | `3`        | Seconds to sleep between polls when idle |
| `--tries`         | `3`        | Max attempts per job (overrides per-job) |
| `--timeout`       | `60`       | Seconds before a job is considered stuck |

```bash
# Process the "mail" queue, sleep 5s between polls
ravel queue:work --queue mail --sleep 5

# Process with a global retry limit of 10
ravel queue:work --tries 10
```

### Graceful Shutdown

Press **Ctrl+C** to initiate a graceful shutdown. The worker completes its
current job before exiting, ensuring no work is lost mid-execution.

```bash
ravel queue:work
^C Shutting down gracefully... finishing current job
```

## Programmatic Worker

You don't need the CLI — drive the queue programmatically from your own
binary:

```rust
use ravel_support::queue::Queue;

let queue = Queue::memory();
queue.register::<SendWelcomeEmail>();

// Process one job
let processed = queue.work().await;

// Process all pending jobs
let count = queue.run().await;

// Check how many jobs are waiting
let pending = queue.pending().await;
```

## Retry Logic

When `handle()` returns `Err`, the worker increments the attempt counter and
checks it against `max_attempts`:

1. **Attempts < max_attempts** — the job is re-queued for another try.
2. **Attempts >= max_attempts** — the job is moved to the **dead-letter
   queue** (failed-jobs storage).

```rust
#[derive(Serialize, Deserialize, Job)]
#[job(name = "process_payment", max_attempts = 5)]
struct ProcessPayment {
    order_id: u32,
}

#[async_trait]
impl Job for ProcessPayment {
    async fn handle(&self) -> anyhow::Result<()> {
        // If this fails, the worker retries up to 5 times
        charge_gateway(self.order_id).await?;
        Ok(())
    }
}
```

Each retry logs a warning with the attempt number and error details so you can
monitor progress.

## Failed Job Tracking

When a job exhausts its retries, it becomes a `FailedJob` record accessible via
the queue:

```rust
use ravel_support::queue::Queue;

let queue = Queue::memory();

// Inspect permanently-failed jobs
for failed in queue.failed() {
    println!("Job {} failed after {} attempts: {}",
        failed.job_type, failed.attempts, failed.error);
}

// Re-dispatch a failed job back to the queue
queue.retry_failed(&failed_job_id).await?;
```

The `FailedJob` struct contains:

| Field        | Description                            |
|--------------|----------------------------------------|
| `id`         | Unique job identifier (UUID)           |
| `job_type`   | Job name used for routing              |
| `payload`    | Serialised JSON payload                |
| `error`      | Error message from the last failure    |
| `failed_at`  | Timestamp of permanent failure         |
| `attempts`   | Total attempts before giving up        |

## Dead-Letter Queue

The dead-letter queue (DLQ) is the collection of permanently-failed jobs.
In the in-memory driver these are kept in a `Vec<FailedJob>` on the `Queue`
struct. Production drivers (Redis, database) persist them externally so
operators can inspect, replay, or alert on failures.

## Example: Worker with Retries

```rust
use ravel_support::queue::{Queue, Job};
use ravel_macros::Job;
use serde::{Serialize, Deserialize};
use async_trait::async_trait;

#[derive(Serialize, Deserialize, Job)]
#[job(name = "send_notification", max_attempts = 5)]
struct SendNotification {
    user_id: u32,
    message: String,
}

#[async_trait]
impl Job for SendNotification {
    async fn handle(&self) -> anyhow::Result<()> {
        // This may fail transiently (network, rate limit)
        send_push_notification(self.user_id, &self.message).await?;
        Ok(())
    }
}

async fn run_worker() {
    let queue = Queue::memory();
    queue.register::<SendNotification>();

    loop {
        if !queue.work().await {
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        }
    }
}
```

When `send_push_notification` fails, the worker logs the error, increments the
attempt, and re-queues. On the 5th failure the job moves to the dead-letter
queue for manual inspection.
