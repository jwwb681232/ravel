//! HTTP server — start an Axum server from a [`Router`].
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_http::server;
//! use axum::Router;
//!
//! let router = Router::new();
//! server::serve(router, "127.0.0.1:3000").await?;
//! ```

use anyhow::{Context, Result};
use axum::Router;

/// Start the HTTP server, binding to `addr` (e.g. `"127.0.0.1:3000"`).
///
/// This is a convenience wrapper around `tokio::net::TcpListener` and
/// `axum::serve`.  Returns when the server shuts down.
pub async fn serve(router: Router, addr: &str) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind to {addr}"))?;

    println!("Server running at http://{addr}");

    axum::serve(listener, router)
        .await
        .with_context(|| "Server error")?;

    Ok(())
}
