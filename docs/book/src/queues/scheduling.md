# Scheduling

The **Scheduler** provides a cron-style API for defining recurring tasks that
run inside your application process. It is designed for lightweight periodic
work such as cleanup, report generation, and heartbeat checks.

## Creating a Scheduler

```rust
use ravel_support::scheduler::Scheduler;

let mut sched = Scheduler::new();
```

By default the scheduler uses an in-memory driver that tracks last-run
timestamps. For persistence across restarts, provide a custom
`SchedulerDriver`.

## Defining Recurring Tasks

Use the `call()` method to register a closure, then chain an interval method:

```rust
sched.call("cleanup_temp_files", || {
    // Delete expired temp files
    std::fs::remove_dir_all("/tmp/expired").ok();
})
.every_minutes(30);
```

### Interval Methods

| Method            | Description                          |
|-------------------|--------------------------------------|
| `every_minutes(n)`| Run every `n` minutes                |
| `every_hours(n)`  | Run every `n` hours                  |
| `daily()`         | Run once per day (every 24 hours)    |

All methods accept a closure with `Fn() + Send + Sync + 'static`. Each returns
the task name as a `String`.

```rust
sched.call("hourly_report", || {
    generate_report().ok();
})
.every_hours(1);

sched.call("daily_cleanup", || {
    cleanup_logs();
    rotate_sessions();
})
.daily();
```

## Running the Scheduler

Call `tick()` periodically — typically in a background Tokio task or inside
your application's main loop:

```rust
use std::time::Duration;

let sched = Arc::new(sched);

tokio::spawn({
    let sched = sched.clone();
    async move {
        loop {
            sched.tick();
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    }
});
```

`tick()` checks every registered task. If the current time is past the task's
`next_run`, the closure is invoked and `next_run` is advanced by the task's
interval. Tasks that are not yet due are skipped.

## The SchedulerDriver Trait

Implement `SchedulerDriver` to persist task execution times in Redis, a
database, or any external store:

```rust
use ravel_support::scheduler::SchedulerDriver;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

struct RedisSchedulerDriver {
    // ...
}

#[async_trait]
impl SchedulerDriver for RedisSchedulerDriver {
    async fn heartbeat(&self, task_id: &str, at: DateTime<Utc>) -> anyhow::Result<()> {
        // Store `at` for `task_id` in Redis
        Ok(())
    }

    async fn last_run(&self, task_id: &str) -> anyhow::Result<Option<DateTime<Utc>>> {
        // Retrieve the last execution timestamp for `task_id` from Redis
        Ok(None)
    }
}
```

Pass the custom driver when constructing the scheduler:

```rust
let driver = RedisSchedulerDriver::new(/* ... */);
let mut sched = Scheduler::with_driver(driver);
```

This way, if your application restarts, the scheduler picks up where it left
off rather than re-running missed tasks.

## Example: Daily Cleanup Task

```rust
use ravel_support::scheduler::Scheduler;
use std::sync::Arc;

fn build_scheduler() -> Scheduler {
    let mut sched = Scheduler::new();

    // Remove expired sessions every 30 minutes
    sched.call("purge_sessions", || {
        tracing::info!("Purging expired sessions...");
        // session_store::purge_expired().ok();
    })
    .every_minutes(30);

    // Generate daily analytics report at midnight UTC
    sched.call("daily_report", || {
        tracing::info!("Generating daily analytics report...");
        // analytics::generate_report().ok();
    })
    .daily();

    // Ping external health-check every 5 minutes
    sched.call("heartbeat", || {
        // reqwest::blocking::get("https://hc.example.com/ping").ok();
    })
    .every_minutes(5);

    sched
}
```

The scheduler works side-by-side with the job queue — use the scheduler for
recurring tasks and the queue for one-off deferred work.
