//! Queue facade — job queue dispatch.
use anyhow::Result;
use ravel_core::app::app;
use ravel_support::queue::{Job, Queue as QueueEngine};
use std::sync::Arc;

pub struct Queue;

impl Queue {
    fn engine() -> Arc<QueueEngine> {
        let app = app().expect("Application not booted");
        app.container()
            .resolve::<QueueEngine>()
            .expect("Queue not registered — use ApplicationExt::with_queue() before boot")
    }

    pub fn dispatch<J: Job + serde::Serialize>(job: J) -> Result<()> {
        let engine = Self::engine();
        tokio::spawn(async move {
            if let Err(e) = engine.dispatch(job).await {
                tracing::error!("Queue dispatch failed: {e}");
            }
        });
        Ok(())
    }

    pub fn dispatch_later<J: Job + serde::Serialize>(
        job: J,
        delay: chrono::Duration,
    ) -> Result<()> {
        let engine = Self::engine();
        tokio::spawn(async move {
            if let Err(e) = engine.dispatch_later(job, delay).await {
                tracing::error!("Queue dispatch_later failed: {e}");
            }
        });
        Ok(())
    }

    pub fn pending() -> Result<usize> {
        let engine = Self::engine();
        Ok(tokio::runtime::Handle::current().block_on(engine.pending()))
    }
}
