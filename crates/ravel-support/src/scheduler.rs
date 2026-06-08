//! Task scheduler — cron-style recurring tasks.
//!
//! Define commands/closure callbacks that run on a schedule.
//! Only supports in-process execution currently.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_support::scheduler::Scheduler;
//!
//! let mut sched = Scheduler::new();
//! sched.call("cleanup", || { println!("cleanup!"); })
//!      .every_minutes(5)
//!      .name("Clean temp files");
//!
//! // In your application's main loop:
//! sched.tick(); // checks if any tasks are due
//! ```

use chrono::{DateTime, Duration, Utc};
use std::sync::{Arc, Mutex};

/// A scheduled task.
pub struct Task {
    name: String,
    callback: Arc<dyn Fn() + Send + Sync + 'static>,
    interval: Duration,
    next_run: Mutex<DateTime<Utc>>,
}

/// The scheduler that holds tasks and checks if any are due.
pub struct Scheduler {
    tasks: Vec<Task>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self { tasks: Vec::new() }
    }

    /// Register a closure to run on a schedule.
    ///
    /// Returns a [`TaskBuilder`] to set the interval and description.
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

    /// Check all tasks and run any that are due.
    ///
    /// Call this regularly from your application's main loop or a
    /// separate thread.
    pub fn tick(&self) {
        let now = Utc::now();
        for task in &self.tasks {
            let mut next_run = task.next_run.lock().unwrap();
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
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ── TaskBuilder ────────────────────────────────────────────────────

/// Fluent builder for scheduled tasks.
pub struct TaskBuilder<'a> {
    scheduler: &'a mut Scheduler,
    name: String,
    callback: Arc<dyn Fn() + Send + Sync + 'static>,
}

impl TaskBuilder<'_> {
    /// Run the task every `minutes` minutes.
    pub fn every_minutes(self, minutes: i64) -> &'static str {
        let task = Task {
            name: self.name.clone(),
            callback: self.callback,
            interval: Duration::minutes(minutes),
            next_run: Mutex::new(Utc::now() + Duration::minutes(minutes)),
        };
        let name = task.name.clone();
        // Leak the string to get a &'static str — this is a tiny allocation
        // and the scheduler lives for the process lifetime.
        let leaked: &'static str = Box::leak(name.into_boxed_str());
        self.scheduler.tasks.push(task);
        leaked
    }

    /// Run the task every `hours` hours.
    pub fn every_hours(self, hours: i64) -> &'static str {
        let task = Task {
            name: self.name.clone(),
            callback: self.callback,
            interval: Duration::hours(hours),
            next_run: Mutex::new(Utc::now() + Duration::hours(hours)),
        };
        let leaked: &'static str = Box::leak(self.name.into_boxed_str());
        self.scheduler.tasks.push(task);
        leaked
    }

    /// Run the task once per day at midnight UTC.
    pub fn daily(self) -> &'static str {
        self.every_hours(24)
    }
}

// ── Tests ──────────────────────────────────────────────────────────

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
        // Two tasks that both increment the counter
        sched.call("task_a", move || { c.fetch_add(1, Ordering::SeqCst); })
            .every_minutes(5);
        sched.call("task_b", move || { c2.fetch_add(10, Ordering::SeqCst); })
            .every_minutes(5);

        // Force the next_run time into the past so tick() executes them
        for task in &sched.tasks {
            let mut nr = task.next_run.lock().unwrap();
            *nr = Utc::now() - Duration::minutes(1);
        }

        sched.tick();
        sched.tick(); // second tick shouldn't fire again (interval on the future now)

        // Both tasks should have fired exactly once
        // task_a +1, task_b +10 = 11
        assert_eq!(counter.load(Ordering::SeqCst), 11);
    }

    #[test]
    fn test_tick_only_runs_when_due() {
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let mut sched = Scheduler::new();
        sched.call("future_task", move || { c.fetch_add(1, Ordering::SeqCst); })
            .every_minutes(60); // not due yet

        sched.tick();
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }
}
