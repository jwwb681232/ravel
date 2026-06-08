//! Queue system — job abstraction with pluggable drivers.
//!
//! Jobs implement the [`Job`] trait; the [`Queue`] dispatches them.
//! Drivers can be swapped: in-memory, Redis, or database-backed.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_support::queue::{Job, Queue, JobPayload};
//! use serde::{Serialize, Deserialize};
//! use async_trait::async_trait;
//!
//! #[derive(Serialize, Deserialize)]
//! struct SendEmail { to: String, body: String }
//!
//! #[async_trait]
//! impl Job for SendEmail {
//!     async fn handle(&self) -> anyhow::Result<()> {
//!         println!("Sending email to {}", self.to);
//!         Ok(())
//!     }
//!     fn name() -> &'static str { "send_email" }
//! }
//!
//! let queue = Queue::memory();
//! queue.register::<SendEmail>();
//! queue.dispatch(SendEmail { to: "a@x.com".into(), body: "Hi!".into() }).await.unwrap();
//! queue.run().await;
//! ```

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tracing::{error, warn};
use uuid::Uuid;

// ── JobPayload ──────────────────────────────────────────────────────────

/// Metadata-rich job envelope stored in the queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobPayload {
    /// Unique job identifier.
    pub id: Uuid,
    /// The job type name (used for routing to handlers).
    pub job_type: String,
    /// Serialised JSON payload.
    pub payload: String,
    /// Number of times this job has been attempted.
    pub attempts: u32,
    /// Maximum attempts before moving to failed-jobs.
    pub max_attempts: u32,
    /// If set, don't process before this time.
    pub delay_until: Option<DateTime<Utc>>,
    /// When the job was created.
    pub created_at: DateTime<Utc>,
    /// Queue name this job belongs to.
    pub queue: String,
}

impl JobPayload {
    /// Create a new payload for the given job type.
    pub fn new(job_type: impl Into<String>, payload: impl Into<String>, max_attempts: u32) -> Self {
        Self {
            id: Uuid::new_v4(),
            job_type: job_type.into(),
            payload: payload.into(),
            attempts: 0,
            max_attempts,
            delay_until: None,
            created_at: Utc::now(),
            queue: "default".into(),
        }
    }

    /// True if the job is ready to be processed (delay has passed).
    pub fn is_ready(&self) -> bool {
        match self.delay_until {
            Some(t) => Utc::now() >= t,
            None => true,
        }
    }
}

// ── Job trait ──────────────────────────────────────────────────────────

/// A unit of work that can be dispatched to a queue.
///
/// Implementors should derive `Serialize + Deserialize`.
#[async_trait]
pub trait Job: Serialize + DeserializeOwned + Send + Sync + 'static {
    /// Execute the job logic. Return `Err` to trigger a retry or failure.
    async fn handle(&self) -> Result<()>;

    /// Unique job name used for routing.
    fn name() -> &'static str;

    /// Maximum retry attempts (default: 3).
    fn max_attempts() -> u32 {
        3
    }

    /// Queue name this job should be routed to (default: "default").
    fn queue() -> &'static str {
        "default"
    }
}

// ── QueueDriver trait ───────────────────────────────────────────────────

/// Pluggable queue backend.
#[async_trait]
pub trait QueueDriver: Send + Sync {
    /// Push a job onto the queue.
    async fn push(&self, job: JobPayload) -> Result<()>;

    /// Pop the next ready job from a queue.
    async fn pop(&self, queue: &str) -> Result<Option<JobPayload>>;

    /// Acknowledge successful processing (remove from queue).
    async fn ack(&self, job_id: &Uuid) -> Result<()>;

    /// Negative-acknowledge: optionally requeue for retry.
    async fn nack(&self, job_id: &Uuid, requeue: bool) -> Result<()>;

    /// Number of pending jobs in a queue.
    async fn size(&self, queue: &str) -> Result<usize>;

    /// Re-queue a job with incremented attempt count.
    async fn retry(&self, job: JobPayload) -> Result<()> {
        let mut job = job;
        job.attempts += 1;
        self.push(job).await
    }
}

// ── In-memory driver ───────────────────────────────────────────────────

struct MemoryDriver {
    jobs: Mutex<VecDeque<JobPayload>>,
}

impl MemoryDriver {
    fn new() -> Self {
        Self {
            jobs: Mutex::new(VecDeque::new()),
        }
    }
}

#[async_trait]
impl QueueDriver for MemoryDriver {
    async fn push(&self, job: JobPayload) -> Result<()> {
        self.jobs.lock().unwrap().push_back(job);
        Ok(())
    }

    async fn pop(&self, _queue: &str) -> Result<Option<JobPayload>> {
        let mut jobs = self.jobs.lock().unwrap();
        // Find first ready job
        if let Some(pos) = jobs.iter().position(|j| j.is_ready()) {
            Ok(Some(jobs.remove(pos).unwrap()))
        } else {
            Ok(None)
        }
    }

    async fn ack(&self, _job_id: &Uuid) -> Result<()> {
        // In-memory: already removed on pop, nothing to do
        Ok(())
    }

    async fn nack(&self, _job_id: &Uuid, _requeue: bool) -> Result<()> {
        // In-memory: already removed on pop
        Ok(())
    }

    async fn size(&self, _queue: &str) -> Result<usize> {
        Ok(self.jobs.lock().unwrap().len())
    }
}

// ── Job Registry ──────────────────────────────────────────────────────

type JobHandler = Arc<
    dyn Fn(&str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>
        + Send
        + Sync,
>;

pub struct JobRegistry {
    handlers: HashMap<String, JobHandler>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn register<J: Job>(&mut self) {
        let handler: JobHandler = Arc::new(|payload: &str| {
            let job: J = serde_json::from_str(payload).expect("Job deserialization failed");
            Box::pin(async move { job.handle().await })
        });
        self.handlers.insert(J::name().to_string(), handler);
    }

    pub fn has(&self, name: &str) -> bool {
        self.handlers.contains_key(name)
    }

    fn get(&self, name: &str) -> Option<&JobHandler> {
        self.handlers.get(name)
    }
}

impl Default for JobRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Failed Job ────────────────────────────────────────────────────────

/// A job that exhausted all retry attempts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedJob {
    pub id: Uuid,
    pub job_type: String,
    pub payload: String,
    pub error: String,
    pub failed_at: DateTime<Utc>,
    pub attempts: u32,
}

// ── Queue ──────────────────────────────────────────────────────────────

/// The job queue with driver and registry.
pub struct Queue {
    driver: Arc<dyn QueueDriver>,
    registry: Arc<Mutex<JobRegistry>>,
    failed_jobs: Arc<Mutex<Vec<FailedJob>>>,
}

impl Queue {
    /// Create a queue backed by the in-memory driver.
    pub fn memory() -> Self {
        Self {
            driver: Arc::new(MemoryDriver::new()),
            registry: Arc::new(Mutex::new(JobRegistry::new())),
            failed_jobs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create a queue with a custom driver.
    pub fn with_driver(driver: impl QueueDriver + 'static) -> Self {
        Self {
            driver: Arc::new(driver),
            registry: Arc::new(Mutex::new(JobRegistry::new())),
            failed_jobs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Register a job type.
    pub fn register<J: Job>(&self) {
        self.registry.lock().unwrap().register::<J>();
    }

    /// Dispatch a job.
    pub async fn dispatch<J: Job>(&self, job: J) -> Result<()> {
        let payload = serde_json::to_string(&job)?;
        let mut job_payload = JobPayload::new(J::name(), payload, J::max_attempts());
        job_payload.queue = J::queue().into();
        self.driver.push(job_payload).await
    }

    /// Dispatch a job with a delay.
    pub async fn dispatch_later<J: Job>(&self, job: J, delay: chrono::Duration) -> Result<()> {
        let payload = serde_json::to_string(&job)?;
        let mut job_payload = JobPayload::new(J::name(), payload, J::max_attempts());
        job_payload.queue = J::queue().into();
        job_payload.delay_until = Some(Utc::now() + delay);
        self.driver.push(job_payload).await
    }

    /// Process one job. Returns `true` if a job was processed.
    pub async fn work(&self) -> bool {
        // Simple round-robin across default queue
        if let Ok(Some(job)) = self.driver.pop("default").await {
            self.process_job(job).await;
            return true;
        }
        false
    }

    /// Process all pending jobs. Returns the count processed.
    pub async fn run(&self) -> usize {
        let mut count = 0;
        while self.work().await {
            count += 1;
        }
        count
    }

    /// Number of pending jobs.
    pub async fn pending(&self) -> usize {
        self.driver.size("default").await.unwrap_or(0)
    }

    /// Check if a job type is registered.
    pub fn is_registered<J: Job>(&self) -> bool {
        self.registry.lock().unwrap().has(J::name())
    }

    /// List failed jobs.
    pub fn failed(&self) -> Vec<FailedJob> {
        self.failed_jobs.lock().unwrap().clone()
    }

    /// Re-dispatch a failed job by ID.
    pub async fn retry_failed(&self, id: &Uuid) -> Result<()> {
        let job = {
            let mut failed = self.failed_jobs.lock().unwrap();
            let pos = failed.iter().position(|j| &j.id == id);
            match pos {
                Some(p) => failed.remove(p),
                None => anyhow::bail!("Failed job {id} not found"),
            }
        };
        let job_payload = JobPayload {
            id: job.id,
            job_type: job.job_type,
            payload: job.payload,
            attempts: 0,
            max_attempts: 3,
            delay_until: None,
            created_at: Utc::now(),
            queue: "default".into(),
        };
        self.driver.push(job_payload).await
    }

    /// Get the driver (for custom operations).
    pub fn driver(&self) -> &Arc<dyn QueueDriver> {
        &self.driver
    }

    // ── internal ──────────────────────────────────────────────────

    async fn process_job(&self, job: JobPayload) {
        let handler = {
            let registry = self.registry.lock().unwrap();
            match registry.get(&job.job_type) {
                Some(h) => h.clone(),
                None => {
                    warn!("unregistered job type '{}', skipping", job.job_type);
                    return;
                }
            }
        };

        match handler(&job.payload).await {
            Ok(()) => {
                let _ = self.driver.ack(&job.id).await;
            }
            Err(e) => {
                let error_msg = format!("{e:?}");
                error!(
                    "job '{}' ({}) attempt {}/{} failed: {error_msg}",
                    job.job_type,
                    job.id,
                    job.attempts + 1,
                    job.max_attempts
                );

                if job.attempts + 1 >= job.max_attempts {
                    // Exhausted retries — move to failed jobs
                    self.failed_jobs.lock().unwrap().push(FailedJob {
                        id: job.id,
                        job_type: job.job_type.clone(),
                        payload: job.payload.clone(),
                        error: error_msg,
                        failed_at: Utc::now(),
                        attempts: job.attempts + 1,
                    });
                    error!(
                        "job '{}' ({}) permanently failed after {} attempts",
                        job.job_type,
                        job.id,
                        job.attempts + 1
                    );
                } else {
                    // Retry
                    let _ = self.driver.retry(job).await;
                }
            }
        }
    }
}

impl Default for Queue {
    fn default() -> Self {
        Self::memory()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER_SINGLE: AtomicUsize = AtomicUsize::new(0);
    static COUNTER_BATCH: AtomicUsize = AtomicUsize::new(0);
    static COUNTER_MULTI_A: AtomicUsize = AtomicUsize::new(0);
    static COUNTER_MULTI_B: AtomicUsize = AtomicUsize::new(0);

    #[derive(Serialize, Deserialize)]
    struct SingleJob {
        msg: String,
    }
    #[async_trait]
    impl Job for SingleJob {
        async fn handle(&self) -> Result<()> {
            COUNTER_SINGLE.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn name() -> &'static str {
            "single_job"
        }
    }

    #[derive(Serialize, Deserialize)]
    struct BatchJob {
        msg: String,
    }
    #[async_trait]
    impl Job for BatchJob {
        async fn handle(&self) -> Result<()> {
            COUNTER_BATCH.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn name() -> &'static str {
            "batch_job"
        }
    }

    #[derive(Serialize, Deserialize)]
    struct MultiJobA {
        msg: String,
    }
    #[async_trait]
    impl Job for MultiJobA {
        async fn handle(&self) -> Result<()> {
            COUNTER_MULTI_A.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn name() -> &'static str {
            "multi_job_a"
        }
    }

    #[derive(Serialize, Deserialize)]
    struct MultiJobB {
        value: usize,
    }
    #[async_trait]
    impl Job for MultiJobB {
        async fn handle(&self) -> Result<()> {
            COUNTER_MULTI_B.fetch_add(self.value, Ordering::SeqCst);
            Ok(())
        }
        fn name() -> &'static str {
            "multi_job_b"
        }
    }

    #[derive(Serialize, Deserialize)]
    struct UnregJob {
        msg: String,
    }
    #[async_trait]
    impl Job for UnregJob {
        async fn handle(&self) -> Result<()> {
            panic!("should not be called");
        }
        fn name() -> &'static str {
            "unreg_job"
        }
    }

    #[derive(Serialize, Deserialize)]
    struct FailingJob;
    #[async_trait]
    impl Job for FailingJob {
        async fn handle(&self) -> Result<()> {
            anyhow::bail!("always fails");
        }
        fn name() -> &'static str {
            "failing_job"
        }
        fn max_attempts() -> u32 {
            2
        }
    }

    #[tokio::test]
    async fn test_dispatch_and_work() {
        COUNTER_SINGLE.store(0, Ordering::SeqCst);
        let queue = Queue::memory();
        queue.register::<SingleJob>();
        assert_eq!(queue.pending().await, 0);

        queue
            .dispatch(SingleJob {
                msg: "hello".into(),
            })
            .await
            .unwrap();
        assert_eq!(queue.pending().await, 1);

        assert!(queue.work().await);
        assert_eq!(queue.pending().await, 0);
        assert_eq!(COUNTER_SINGLE.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_run_processes_all() {
        COUNTER_BATCH.store(0, Ordering::SeqCst);
        let queue = Queue::memory();
        queue.register::<BatchJob>();

        for _ in 0..5 {
            queue.dispatch(BatchJob { msg: "x".into() }).await.unwrap();
        }

        let count = queue.run().await;
        assert_eq!(count, 5);
        assert_eq!(queue.pending().await, 0);
        assert_eq!(COUNTER_BATCH.load(Ordering::SeqCst), 5);
    }

    #[tokio::test]
    async fn test_multiple_job_types() {
        COUNTER_MULTI_A.store(0, Ordering::SeqCst);
        COUNTER_MULTI_B.store(0, Ordering::SeqCst);
        let queue = Queue::memory();
        queue.register::<MultiJobA>();
        queue.register::<MultiJobB>();

        queue.dispatch(MultiJobA { msg: "a".into() }).await.unwrap();
        queue.dispatch(MultiJobB { value: 10 }).await.unwrap();
        queue.dispatch(MultiJobA { msg: "b".into() }).await.unwrap();

        let count = queue.run().await;
        assert_eq!(count, 3);
        assert_eq!(COUNTER_MULTI_A.load(Ordering::SeqCst), 2);
        assert_eq!(COUNTER_MULTI_B.load(Ordering::SeqCst), 10);
    }

    #[tokio::test]
    async fn test_unregistered_job_skipped() {
        let queue = Queue::memory();
        queue.dispatch(UnregJob { msg: "x".into() }).await.unwrap();
        assert!(queue.work().await);
    }

    #[tokio::test]
    async fn test_is_registered() {
        let queue = Queue::memory();
        assert!(!queue.is_registered::<SingleJob>());
        queue.register::<SingleJob>();
        assert!(queue.is_registered::<SingleJob>());
        assert!(!queue.is_registered::<BatchJob>());
    }

    #[tokio::test]
    async fn test_failing_job_goes_to_failed() {
        let queue = Queue::memory();
        queue.register::<FailingJob>();
        queue.dispatch(FailingJob).await.unwrap();

        // Process — will fail, retry, then permanently fail
        queue.run().await;

        let failed = queue.failed();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].job_type, "failing_job");
        assert_eq!(failed[0].attempts, 2); // max_attempts = 2
    }

    #[tokio::test]
    async fn test_dispatch_later_delays_job() {
        COUNTER_SINGLE.store(0, Ordering::SeqCst);
        let queue = Queue::memory();
        queue.register::<SingleJob>();

        queue
            .dispatch_later(
                SingleJob {
                    msg: "delayed".into(),
                },
                chrono::Duration::minutes(60),
            )
            .await
            .unwrap();

        // Job should not be ready yet
        let processed = queue.work().await;
        assert!(!processed);
        assert_eq!(COUNTER_SINGLE.load(Ordering::SeqCst), 0);
    }
}
