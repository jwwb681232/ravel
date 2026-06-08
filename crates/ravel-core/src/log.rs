//! Structured logging via `tracing`.
//!
//! Initialise with [`Log::init()`] during application bootstrap. After that
//! all crates use `tracing::info!`, `tracing::error!`, etc.
//!
//! ```rust,ignore
//! use ravel_core::log::Log;
//! Log::init("info");
//! tracing::info!("Application started");
//! ```

use std::sync::OnceLock;
use tracing_subscriber::EnvFilter;

/// Logger initialisation helper.
pub struct Log;

static INIT: OnceLock<()> = OnceLock::new();

impl Log {
    /// Initialise the tracing subscriber with the given default log level.
    ///
    /// The `RAVEL_LOG` environment variable takes precedence over `default_level`.
    /// This is idempotent — subsequent calls are no-ops.
    ///
    /// In non-debug builds, output is JSON (machine-readable). In debug
    /// builds, output is human-readable text.
    pub fn init(default_level: &str) {
        INIT.get_or_init(|| {
            let filter = EnvFilter::try_from_env("RAVEL_LOG")
                .unwrap_or_else(|_| EnvFilter::new(default_level));

            let subscriber = tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_target(false);

            if cfg!(debug_assertions) {
                subscriber.init();
            } else {
                subscriber.json().init();
            }
        });
    }
}

/// Re-export the tracing macros for convenience.
pub use tracing::{debug, error, info, trace, warn};
