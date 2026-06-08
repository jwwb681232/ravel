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
//! use std::sync::Arc;
//! use std::sync::atomic::{AtomicUsize, Ordering};
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
//! queue.register::<SendEmail>();
//! queue.dispatch(SendEmail { to: "alice@x.com".into(), body: "Hi!".into() }).unwrap();
//! queue.run();
//! ```

use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::{HashMap, VecDeque};
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

// ── Job Registry ───────────────────────────────────────────────────

/// A handler function that deserializes a job payload and executes it.
pub type JobHandler = Arc<dyn Fn(&str) -> Result<()> + Send + Sync>;

/// Registry that maps job names to their handler functions.
///
/// When a job is popped from the queue, the registry looks up the
/// handler by name, deserializes the payload, and calls `handle()`.
pub struct JobRegistry {
    handlers: HashMap<String, JobHandler>,
}

impl JobRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a job type. This allows the queue to deserialize
    /// and execute jobs of this type when they are popped.
    pub fn register<J: Job>(&mut self) {
        let handler: JobHandler = Arc::new(|payload: &str| {
            let job: J = serde_json::from_str(payload)?;
            job.handle();
            Ok(())
        });
        self.handlers.insert(J::name().to_string(), handler);
    }

    /// Check if a job type is registered.
    pub fn has(&self, name: &str) -> bool {
        self.handlers.contains_key(name)
    }

    /// Get the handler for a job name.
    fn get(&self, name: &str) -> Option<&JobHandler> {
        self.handlers.get(name)
    }
}

impl Default for JobRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Queue ──────────────────────────────────────────────────────────

/// The job queue.
///
/// Holds a driver (memory, Redis, etc.) and a registry of job handlers.
/// Jobs must be registered with [`register`](Self::register) before they
/// can be executed by [`work`](Self::work).
pub struct Queue {
    driver: Arc<dyn QueueDriver>,
    registry: Arc<Mutex<JobRegistry>>,
}

impl Queue {
    /// Create a queue backed by the in-memory driver.
    pub fn memory() -> Self {
        Self {
            driver: Arc::new(MemoryDriver {
                jobs: Mutex::new(VecDeque::new()),
            }),
            registry: Arc::new(Mutex::new(JobRegistry::new())),
        }
    }

    /// Create a queue with a custom driver.
    pub fn with_driver(driver: impl QueueDriver + 'static) -> Self {
        Self {
            driver: Arc::new(driver),
            registry: Arc::new(Mutex::new(JobRegistry::new())),
        }
    }

    /// Register a job type so it can be executed when popped from the queue.
    ///
    /// You must call this for each job type before calling `work()` or `run()`.
    pub fn register<J: Job>(&self) {
        self.registry.lock().unwrap().register::<J>();
    }

    /// Dispatch a job by serialising it and pushing it onto the queue.
    pub fn dispatch<J: Job>(&self, job: J) -> Result<()> {
        let payload = serde_json::to_string(&job)?;
        self.driver.push(J::name(), &payload)
    }

    /// Process the next job in the queue, returning true if a job was run.
    ///
    /// Panics if the job type is not registered.
    pub fn work(&self) -> bool {
        if let Ok(Some((name, payload))) = self.driver.pop() {
            let registry = self.registry.lock().unwrap();
            if let Some(handler) = registry.get(&name) {
                handler(&payload).unwrap_or_else(|e| {
                    eprintln!("[queue] error executing job '{}': {}", name, e);
                });
            } else {
                eprintln!(
                    "[queue] warning: job type '{}' not registered, skipping",
                    name
                );
            }
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

    /// Check if a job type is registered.
    pub fn is_registered<J: Job>(&self) -> bool {
        self.registry.lock().unwrap().has(J::name())
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Each test gets its own job type + counter to avoid parallel test interference
    static COUNTER_SINGLE: AtomicUsize = AtomicUsize::new(0);
    static COUNTER_BATCH: AtomicUsize = AtomicUsize::new(0);
    static COUNTER_MULTI_A: AtomicUsize = AtomicUsize::new(0);
    static COUNTER_MULTI_B: AtomicUsize = AtomicUsize::new(0);

    #[derive(Serialize, Deserialize)]
    struct SingleJob { msg: String }
    impl Job for SingleJob {
        fn handle(&self) { COUNTER_SINGLE.fetch_add(1, Ordering::SeqCst); }
        fn name() -> &'static str { "single_job" }
    }

    #[derive(Serialize, Deserialize)]
    struct BatchJob { msg: String }
    impl Job for BatchJob {
        fn handle(&self) { COUNTER_BATCH.fetch_add(1, Ordering::SeqCst); }
        fn name() -> &'static str { "batch_job" }
    }

    #[derive(Serialize, Deserialize)]
    struct MultiJobA { msg: String }
    impl Job for MultiJobA {
        fn handle(&self) { COUNTER_MULTI_A.fetch_add(1, Ordering::SeqCst); }
        fn name() -> &'static str { "multi_job_a" }
    }

    #[derive(Serialize, Deserialize)]
    struct MultiJobB { value: usize }
    impl Job for MultiJobB {
        fn handle(&self) { COUNTER_MULTI_B.fetch_add(self.value, Ordering::SeqCst); }
        fn name() -> &'static str { "multi_job_b" }
    }

    #[derive(Serialize, Deserialize)]
    struct UnregJob { msg: String }
    impl Job for UnregJob {
        fn handle(&self) { panic!("should not be called"); }
        fn name() -> &'static str { "unreg_job" }
    }

    #[test]
    fn test_dispatch_and_work() {
        COUNTER_SINGLE.store(0, Ordering::SeqCst);
        let queue = Queue::memory();
        queue.register::<SingleJob>();
        assert_eq!(queue.pending(), 0);

        queue.dispatch(SingleJob { msg: "hello".into() }).unwrap();
        assert_eq!(queue.pending(), 1);

        assert!(queue.work());
        assert_eq!(queue.pending(), 0);
        assert_eq!(COUNTER_SINGLE.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_run_processes_all() {
        COUNTER_BATCH.store(0, Ordering::SeqCst);
        let queue = Queue::memory();
        queue.register::<BatchJob>();

        for _ in 0..5 {
            queue.dispatch(BatchJob { msg: "x".into() }).unwrap();
        }

        let count = queue.run();
        assert_eq!(count, 5);
        assert_eq!(queue.pending(), 0);
        assert_eq!(COUNTER_BATCH.load(Ordering::SeqCst), 5);
    }

    #[test]
    fn test_multiple_job_types() {
        COUNTER_MULTI_A.store(0, Ordering::SeqCst);
        COUNTER_MULTI_B.store(0, Ordering::SeqCst);
        let queue = Queue::memory();
        queue.register::<MultiJobA>();
        queue.register::<MultiJobB>();

        queue.dispatch(MultiJobA { msg: "a".into() }).unwrap();
        queue.dispatch(MultiJobB { value: 10 }).unwrap();
        queue.dispatch(MultiJobA { msg: "b".into() }).unwrap();

        let count = queue.run();
        assert_eq!(count, 3);
        assert_eq!(COUNTER_MULTI_A.load(Ordering::SeqCst), 2);
        assert_eq!(COUNTER_MULTI_B.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn test_unregistered_job_skipped() {
        let queue = Queue::memory();
        queue.dispatch(UnregJob { msg: "x".into() }).unwrap();
        // work() returns true (a job was popped) but handler is not found
        assert!(queue.work());
    }

    #[test]
    fn test_is_registered() {
        let queue = Queue::memory();
        assert!(!queue.is_registered::<SingleJob>());
        queue.register::<SingleJob>();
        assert!(queue.is_registered::<SingleJob>());
        assert!(!queue.is_registered::<BatchJob>());
    }
}
