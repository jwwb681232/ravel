//! Integration tests for RedisDriver — requires a running Redis on localhost:6379.
//! Run with: cargo test -p ravel-support --features redis --test queue_default -- --test-threads=1

use ravel_support::queue::{Job, JobPayload, Queue};

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Serialize, Deserialize)]
struct TestJob {
    n: usize,
}

#[async_trait::async_trait]
impl Job for TestJob {
    async fn handle(&self) -> anyhow::Result<()> {
        COUNTER.fetch_add(self.n, Ordering::SeqCst);
        Ok(())
    }
    fn name() -> &'static str {
        "test_job"
    }
    fn queue() -> &'static str {
        "default"
    }
}

#[derive(Serialize, Deserialize)]
struct FailingJob;

#[async_trait::async_trait]
impl Job for FailingJob {
    async fn handle(&self) -> anyhow::Result<()> {
        anyhow::bail!("always fails")
    }
    fn name() -> &'static str {
        "failing_job"
    }
    fn max_attempts() -> u32 {
        1
    }
    fn queue() -> &'static str {
        "default"
    }
}

async fn setup() -> (Queue, redis::aio::MultiplexedConnection) {
    let client = redis::Client::open("redis://127.0.0.1:6379").unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();
    let _: () = redis::cmd("DEL")
        .arg("ravel:queue:default")
        .arg("ravel:queue:default:delayed")
        .query_async(&mut conn)
        .await
        .unwrap();

    let driver =
        ravel_support::queue_redis::RedisDriver::connect("redis://127.0.0.1:6379")
            .await
            .unwrap();
    let queue = Queue::with_driver(driver);
    (queue, conn)
}

#[tokio::test]
async fn test_push_and_pop() {
    let (queue, _) = setup().await;
    let job = JobPayload::new("test_job", r#"{"n":1}"#, 3);
    queue.driver().push(job).await.unwrap();

    assert_eq!(queue.driver().size("default").await.unwrap(), 1);

    let popped = queue.driver().pop("default").await.unwrap().unwrap();
    assert_eq!(popped.job_type, "test_job");
    assert_eq!(queue.driver().size("default").await.unwrap(), 0);
}

#[tokio::test]
async fn test_dispatch_and_work() {
    COUNTER.store(0, Ordering::SeqCst);
    let (queue, _) = setup().await;
    queue.register::<TestJob>();
    assert_eq!(queue.pending().await, 0);

    queue.dispatch(TestJob { n: 42 }).await.unwrap();
    assert_eq!(queue.pending().await, 1);

    assert!(queue.work().await);
    assert_eq!(queue.pending().await, 0);
    assert_eq!(COUNTER.load(Ordering::SeqCst), 42);
}

#[tokio::test]
async fn test_run_processes_all() {
    COUNTER.store(0, Ordering::SeqCst);
    let (queue, _) = setup().await;
    queue.register::<TestJob>();

    for _ in 0..5 {
        queue.dispatch(TestJob { n: 10 }).await.unwrap();
    }

    let count = queue.run().await;
    assert_eq!(count, 5);
    assert_eq!(queue.pending().await, 0);
    assert_eq!(COUNTER.load(Ordering::SeqCst), 50);
}

#[tokio::test]
async fn test_failing_job_goes_to_failed() {
    let (queue, _) = setup().await;
    queue.register::<FailingJob>();
    queue.dispatch(FailingJob).await.unwrap();

    queue.run().await;

    let failed = queue.failed();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].job_type, "failing_job");
}

#[tokio::test]
async fn test_delayed_job() {
    COUNTER.store(0, Ordering::SeqCst);
    let (queue, _) = setup().await;
    queue.register::<TestJob>();

    queue
        .dispatch_later(TestJob { n: 7 }, chrono::Duration::hours(1))
        .await
        .unwrap();

    assert_eq!(queue.driver().size("default").await.unwrap(), 1);
    let popped = queue.driver().pop("default").await.unwrap();
    assert!(popped.is_none());
    assert_eq!(COUNTER.load(Ordering::SeqCst), 0);
}
