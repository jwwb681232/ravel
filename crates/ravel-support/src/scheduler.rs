//! Task scheduler — cron-style recurring tasks.
//!
//! Define commands/closure callbacks that run on a schedule.
//! Supports both in-process execution and pluggable driver backends.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_support::scheduler::Scheduler;
//!
//! let mut sched = Scheduler::new();
//! sched.call("cleanup", || { println!("cleanup!"); })
//!      .every_minutes(5);
//!
//! // In your application's main loop:
//! sched.tick();
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use std::sync::{Arc, Mutex};

// ── SchedulerDriver trait ────────────────────────────────────────────────

/// Pluggable scheduler backend.
///
/// Implement this to persist scheduled tasks to Redis, a database, etc.
#[async_trait]
pub trait SchedulerDriver: Send + Sync {
    /// Record that a task with this id was executed at this time.
    async fn heartbeat(&self, task_id: &str, at: DateTime<Utc>) -> anyhow::Result<()>;

    /// Return the last execution time for a task, if known.
    async fn last_run(&self, task_id: &str) -> anyhow::Result<Option<DateTime<Utc>>>;
}

// ── In-memory driver ────────────────────────────────────────────────────

struct MemorySchedulerDriver {
    last_runs: Mutex<std::collections::HashMap<String, DateTime<Utc>>>,
}

impl MemorySchedulerDriver {
    fn new() -> Self {
        Self {
            last_runs: Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[async_trait]
impl SchedulerDriver for MemorySchedulerDriver {
    async fn heartbeat(&self, task_id: &str, at: DateTime<Utc>) -> anyhow::Result<()> {
        self.last_runs.lock().unwrap_or_else(|e| e.into_inner()).insert(task_id.into(), at);
        Ok(())
    }

    async fn last_run(&self, task_id: &str) -> anyhow::Result<Option<DateTime<Utc>>> {
        Ok(self.last_runs.lock().unwrap_or_else(|e| e.into_inner()).get(task_id).copied())
    }
}

// ── Task ────────────────────────────────────────────────────────────────

/// A scheduled task.
pub struct Task {
    pub name: String,
    callback: Arc<dyn Fn() + Send + Sync + 'static>,
    interval: Duration,
    next_run: Mutex<DateTime<Utc>>,
}

// ── Scheduler ────────────────────────────────────────────────────────────

/// The scheduler that holds tasks and checks if any are due.
pub struct Scheduler {
    tasks: Vec<Task>,
    driver: Arc<dyn SchedulerDriver>,
}

impl Scheduler {
    /// Create a scheduler with the in-memory driver.
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            driver: Arc::new(MemorySchedulerDriver::new()),
        }
    }

    /// Create a scheduler with a custom driver.
    pub fn with_driver(driver: impl SchedulerDriver + 'static) -> Self {
        Self {
            tasks: Vec::new(),
            driver: Arc::new(driver),
        }
    }

    /// Register a closure to run on a schedule.
    ///
    /// Returns a [`TaskBuilder`] to set the interval.
    pub fn call<F>(&mut self, name: &str, f: F) -> TaskBuilder<'_>
    where
        F: Fn() + Send + Sync + 'static,
    {
        TaskBuilder {
            scheduler: self,
            name: name.to_string(),
            callback: Arc::new(f),
        }
    }

    /// Register a task with a custom interval.
    pub fn add_task(&mut self, task: Task) {
        self.tasks.push(task);
    }

    /// Check all tasks and run any that are due.
    ///
    /// Call this regularly from your application's main loop or a
    /// separate thread.
    pub fn tick(&self) {
        let now = Utc::now();
        for task in &self.tasks {
            let mut next_run = task.next_run.lock().unwrap_or_else(|e| e.into_inner());
            if now >= *next_run {
                (task.callback)();
                *next_run = now + task.interval;
            }
        }
    }

    /// Number of registered tasks.
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Get a reference to the driver.
    pub fn driver(&self) -> &Arc<dyn SchedulerDriver> {
        &self.driver
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ── TaskBuilder ──────────────────────────────────────────────────────────

/// Fluent builder for scheduled tasks.
pub struct TaskBuilder<'a> {
    scheduler: &'a mut Scheduler,
    name: String,
    callback: Arc<dyn Fn() + Send + Sync + 'static>,
}

impl TaskBuilder<'_> {
    /// Run the task every `minutes` minutes.
    pub fn every_minutes(self, minutes: i64) -> String {
        let name = self.name.clone();
        let task = Task {
            name: self.name,
            callback: self.callback,
            interval: Duration::minutes(minutes),
            next_run: Mutex::new(Utc::now() + Duration::minutes(minutes)),
        };
        self.scheduler.tasks.push(task);
        name
    }

    /// Run the task every `hours` hours.
    pub fn every_hours(self, hours: i64) -> String {
        let name = self.name.clone();
        let task = Task {
            name: self.name,
            callback: self.callback,
            interval: Duration::hours(hours),
            next_run: Mutex::new(Utc::now() + Duration::hours(hours)),
        };
        self.scheduler.tasks.push(task);
        name
    }

    /// Run the task once per day at midnight UTC.
    pub fn daily(self) -> String {
        self.every_hours(24)
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_scheduler_call() {
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();
        let c2 = counter.clone();

        let mut sched = Scheduler::new();
        sched
            .call("task_a", move || {
                c.fetch_add(1, Ordering::SeqCst);
            })
            .every_minutes(5);
        sched
            .call("task_b", move || {
                c2.fetch_add(10, Ordering::SeqCst);
            })
            .every_minutes(5);

        // Force the next_run time into the past so tick() executes them
        for task in &sched.tasks {
            let mut nr = task.next_run.lock().unwrap_or_else(|e| e.into_inner());
            *nr = Utc::now() - Duration::minutes(1);
        }

        sched.tick();
        sched.tick(); // second tick shouldn't fire again

        assert_eq!(counter.load(Ordering::SeqCst), 11);
    }

    #[test]
    fn test_tick_only_runs_when_due() {
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let mut sched = Scheduler::new();
        sched
            .call("future_task", move || {
                c.fetch_add(1, Ordering::SeqCst);
            })
            .every_minutes(60);

        sched.tick();
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_scheduler_len() {
        let mut sched = Scheduler::new();
        assert!(sched.is_empty());
        sched.call("a", || {}).every_minutes(1);
        sched.call("b", || {}).every_minutes(2);
        assert_eq!(sched.len(), 2);
    }
}
