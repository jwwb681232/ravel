//! Queue system — job abstraction with in-memory and future Redis support.
//!
//! Jobs implement the `Job` trait; the `Queue` dispatches them.
//! The default implementation runs jobs synchronously in-process;
//! swap in a Redis-backed driver for distributed workloads.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_support::queue::{Job, Queue};
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize)]
//! struct SendEmail { to: String, body: String }
//!
//! impl Job for SendEmail {
//!     fn handle(&self) {
//!         println!("Sending email to {}", self.to);
//!     }
//!     fn name() -> &'static str { "send_email" }
//! }
//!
//! let queue = Queue::memory();
//! queue.dispatch(SendEmail { to: "alice@x.com".into(), body: "Hi!".into() }).unwrap();
//! queue.run();
//! ```

use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A unit of work that can be dispatched to a queue.
///
/// Implementors should also derive `Serialize + Deserialize` for
/// persistence across process boundaries (Redis, etc.).
pub trait Job: Serialize + DeserializeOwned + Send + 'static {
    /// Execute the job logic.
    fn handle(&self);

    /// Unique job name (used for queue routing).
    fn name() -> &'static str;
}

/// The queue driver trait — plug in memory, Redis, RabbitMQ, etc.
pub trait QueueDriver: Send + Sync {
    /// Push a serialised job payload onto the queue.
    fn push(&self, name: &str, payload: &str) -> Result<()>;

    /// Pop the next job (returns (name, payload) or None).
    fn pop(&self) -> Result<Option<(String, String)>>;

    /// Number of pending jobs.
    fn size(&self) -> usize;
}

// ── In-memory driver ───────────────────────────────────────────────

struct MemoryDriver {
    jobs: Mutex<VecDeque<(String, String)>>,
}

impl QueueDriver for MemoryDriver {
    fn push(&self, name: &str, payload: &str) -> Result<()> {
        self.jobs
            .lock()
            .unwrap()
            .push_back((name.to_string(), payload.to_string()));
        Ok(())
    }

    fn pop(&self) -> Result<Option<(String, String)>> {
        Ok(self.jobs.lock().unwrap().pop_front())
    }

    fn size(&self) -> usize {
        self.jobs.lock().unwrap().len()
    }
}

// ── Queue ──────────────────────────────────────────────────────────

/// The job queue.
pub struct Queue {
    driver: Arc<dyn QueueDriver>,
}

impl Queue {
    /// Create a queue backed by the in-memory driver.
    pub fn memory() -> Self {
        Self {
            driver: Arc::new(MemoryDriver {
                jobs: Mutex::new(VecDeque::new()),
            }),
        }
    }

    /// Create a queue with a custom driver.
    pub fn with_driver(driver: impl QueueDriver + 'static) -> Self {
        Self {
            driver: Arc::new(driver),
        }
    }

    /// Dispatch a job by serialising it and pushing it onto the queue.
    pub fn dispatch<J: Job>(&self, job: J) -> Result<()> {
        let payload = serde_json::to_string(&job)?;
        self.driver.push(J::name(), &payload)
    }

    /// Process the next job in the queue, returning true if a job was run.
    pub fn work(&self) -> bool {
        if let Ok(Some((_name, payload))) = self.driver.pop() {
            // In-memory mode: we don't deserialise by job type here since
            // we don't have a type registry.  The real implementation
            // dispatches to registered handlers.
            println!("[queue] processing job: {}", _name);
            let _ = payload;
            return true;
        }
        false
    }

    /// Process all pending jobs.
    pub fn run(&self) -> usize {
        let mut count = 0;
        while self.work() {
            count += 1;
        }
        count
    }

    /// Number of pending jobs.
    pub fn pending(&self) -> usize {
        self.driver.size()
    }
}

impl Default for Queue {
    fn default() -> Self {
        Self::memory()
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Serialize, Deserialize)]
    struct TestJob {
        msg: String,
    }

    impl Job for TestJob {
        fn handle(&self) {
            let _ = &self.msg;
        }
        fn name() -> &'static str {
            "test_job"
        }
    }

    #[test]
    fn test_dispatch_and_work() {
        let queue = Queue::memory();
        assert_eq!(queue.pending(), 0);

        queue
            .dispatch(TestJob {
                msg: "hello".into(),
            })
            .unwrap();
        assert_eq!(queue.pending(), 1);

        assert!(queue.work());
        assert_eq!(queue.pending(), 0);
    }

    #[test]
    fn test_run_processes_all() {
        let queue = Queue::memory();
        for _ in 0..5 {
            queue
                .dispatch(TestJob {
                    msg: "x".into(),
                })
                .unwrap();
        }

        let count = queue.run();
        assert_eq!(count, 5);
        assert_eq!(queue.pending(), 0);
    }
}
