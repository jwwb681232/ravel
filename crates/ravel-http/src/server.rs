//! HTTP server — start an Axum server from a [`Router`].
//!
//! # Usage — quick start
//!
//! ```rust,ignore
//! use ravel_http::server;
//! use axum::Router;
//!
//! let router = Router::new();
//! server::serve(router, "127.0.0.1:3000").await?;
//! ```
//!
//! # Usage — builder with graceful shutdown
//!
//! ```rust,ignore
//! use ravel_http::server::ServerBuilder;
//! use axum::Router;
//!
//! let router = Router::new();
//! ServerBuilder::new(router)
//!     .serve("127.0.0.1:3000")
//!     .with_graceful_shutdown()
//!     .run()
//!     .await?;
//! ```

use anyhow::{Context, Result};
use axum::Router;
use std::future::Future;
use std::pin::Pin;
use tracing::{info, warn};

/// Start the HTTP server, binding to `addr` (e.g. `"127.0.0.1:3000"`).
///
/// Convenience wrapper around [`ServerBuilder`].  For graceful shutdown use
/// [`ServerBuilder`] directly.
pub async fn serve(router: Router, addr: &str) -> Result<()> {
    ServerBuilder::new(router).serve(addr).run().await
}

// ── ServerBuilder ────────────────────────────────────────────────────────

/// Fluent builder for configuring and starting the HTTP server.
///
/// ```rust,ignore
/// ServerBuilder::new(router)
///     .serve("0.0.0.0:3000")
///     .with_graceful_shutdown()
///     .run()
///     .await?;
/// ```
///
/// Stateful routers (created via `Router::with_state(...)`) are supported —
/// pass them directly to [`ServerBuilder::new`].
pub struct ServerBuilder {
    router: Router,
    addr: Option<String>,
    shutdown_signal: Option<Pin<Box<dyn Future<Output = ()> + Send>>>,
}

impl ServerBuilder {
    /// Create a new builder wrapping the given Axum [`Router`].
    pub fn new(router: Router) -> Self {
        Self {
            router,
            addr: None,
            shutdown_signal: None,
        }
    }

    /// Set the listen address (e.g. `"127.0.0.1:3000"`).
    pub fn serve(mut self, addr: impl Into<String>) -> Self {
        self.addr = Some(addr.into());
        self
    }

    /// Register a graceful-shutdown signal.
    ///
    /// The server waits for this future to resolve, then stops accepting new
    /// connections and drains in-flight requests.
    ///
    /// ```rust,ignore
    /// ServerBuilder::new(router)
    ///     .serve("127.0.0.1:3000")
    ///     .with_graceful_shutdown_signal(tokio::signal::ctrl_c())
    ///     .run()
    ///     .await?;
    /// ```
    pub fn with_graceful_shutdown_signal(
        mut self,
        signal: impl Future<Output = ()> + Send + 'static,
    ) -> Self {
        self.shutdown_signal = Some(Box::pin(signal));
        self
    }

    /// Register a Ctrl+C graceful-shutdown handler.
    ///
    /// On platforms where Ctrl+C handling is unavailable this will emit a
    /// warning at startup and skip graceful shutdown.
    pub fn with_graceful_shutdown(self) -> Self {
        self.with_graceful_shutdown_signal(async {
            match tokio::signal::ctrl_c().await {
                Ok(()) => {
                    info!("Shutting down gracefully...");
                }
                Err(e) => {
                    warn!("Ctrl+C handler unavailable: {e}");
                }
            }
        })
    }

    /// Bind and start the server, returning when it shuts down.
    pub async fn run(self) -> Result<()> {
        let addr = self
            .addr
            .as_deref()
            .context("No listen address set — call .serve(addr) before .run()")?;

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .with_context(|| format!("Failed to bind to {addr}"))?;

        info!("Server running at http://{addr}");

        match self.shutdown_signal {
            Some(signal) => {
                axum::serve(listener, self.router)
                    .with_graceful_shutdown(signal)
                    .await
                    .with_context(|| "Server error")?;
            }
            None => {
                axum::serve(listener, self.router)
                    .await
                    .with_context(|| "Server error")?;
            }
        }

        Ok(())
    }
}
