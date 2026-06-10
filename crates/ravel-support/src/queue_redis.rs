//! Redis queue driver — persistent, distributed job queue.
//!
//! Enable with `features = ["redis"]` in your Cargo.toml:
//!
//! ```toml
//! ravel-support = { path = "../ravel-support", features = ["redis"] }
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_support::queue::{Queue, RedisDriver};
//!
//! let driver = RedisDriver::connect("redis://127.0.0.1:6379").await?;
//! let queue = Queue::with_driver(driver);
//! ```
//!
//! # Redis keys
//!
//! | Key pattern | Type | Purpose |
//! |-------------|------|---------|
//! | `ravel:queue:{name}` | LIST | Active job queue (RPUSH / LPOP) |
//! | `ravel:queue:{name}:delayed` | ZSET | Delayed jobs (score = unix timestamp ms) |

use crate::queue::{JobPayload, QueueDriver};
use anyhow::{Context, Result};
use async_trait::async_trait;
use redis::AsyncCommands;
use uuid::Uuid;

/// Redis-backed queue driver.
///
/// Jobs are stored as JSON in Redis lists.  Multiple workers can safely
/// consume from the same Redis instance — LPOP is atomic.
pub struct RedisDriver {
    client: redis::Client,
}

impl RedisDriver {
    /// Connect to a Redis instance.
    ///
    /// `url` should be a Redis connection string, e.g.
    /// `"redis://127.0.0.1:6379"` or `"redis://:password@host:6379/0"`.
    pub async fn connect(url: &str) -> Result<Self> {
        let client =
            redis::Client::open(url).context("Failed to create Redis client")?;
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .context("Failed to connect to Redis")?;
        redis::cmd("PING")
            .query_async::<_, String>(&mut conn)
            .await
            .context("Redis PING failed")?;
        Ok(Self { client })
    }
}

fn queue_key(queue: &str) -> String {
    format!("ravel:queue:{queue}")
}

fn delayed_key(queue: &str) -> String {
    format!("ravel:queue:{queue}:delayed")
}

async fn get_conn(client: &redis::Client) -> Result<impl AsyncCommands> {
    client
        .get_multiplexed_async_connection()
        .await
        .context("Redis connection lost")
}

#[async_trait]
impl QueueDriver for RedisDriver {
    async fn push(&self, job: JobPayload) -> Result<()> {
        let payload = serde_json::to_string(&job)?;
        let mut conn = get_conn(&self.client).await?;

        if let Some(delay_until) = job.delay_until {
            let score = delay_until.timestamp_millis();
            let _: () = conn.zadd(delayed_key(&job.queue), payload, score).await?;
        } else {
            let _: () = conn.rpush(queue_key(&job.queue), payload).await?;
        }
        Ok(())
    }

    async fn pop(&self, queue: &str) -> Result<Option<JobPayload>> {
        let mut conn = get_conn(&self.client).await?;

        // 1. Move any ready delayed jobs to the main queue
        migrate_delayed(&mut conn, queue).await?;

        // 2. LPOP from the main queue
        let raw: Option<String> = conn.lpop(queue_key(queue), None).await?;
        match raw {
            Some(s) => {
                let job: JobPayload = serde_json::from_str(&s)?;
                Ok(Some(job))
            }
            None => Ok(None),
        }
    }

    async fn ack(&self, _job_id: &Uuid) -> Result<()> {
        // Already removed on pop
        Ok(())
    }

    async fn nack(&self, job_id: &Uuid, requeue: bool) -> Result<()> {
        if requeue {
            tracing::warn!(
                "nack with requeue: job {job_id} will be retried via Queue::retry()"
            );
        }
        Ok(())
    }

    async fn size(&self, queue: &str) -> Result<usize> {
        let mut conn = get_conn(&self.client).await?;
        let len: usize = conn.llen(queue_key(queue)).await?;
        let delayed: usize = conn.zcard(delayed_key(queue)).await?;
        Ok(len + delayed)
    }
}

/// Move delayed jobs whose score ≤ now from ZSET to the main LIST.
async fn migrate_delayed(
    conn: &mut impl AsyncCommands,
    queue: &str,
) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let dk = delayed_key(queue);

    let ready: Vec<String> = conn
        .zrangebyscore_limit(&dk, "-inf", now.to_string(), 0, 100)
        .await?;

    if ready.is_empty() {
        return Ok(());
    }

    let qk = queue_key(queue);
    for payload in &ready {
        let _: () = conn.rpush(&qk, payload).await?;
    }
    let ready_refs: Vec<&str> = ready.iter().map(|s| s.as_str()).collect();
    let _: () = conn.zrem(&dk, ready_refs).await?;
    Ok(())
}
