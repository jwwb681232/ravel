//! Laravel-style static facades for the Ravel framework.
//!
//! All facades are usable after Application::boot() has completed.

// Core facades — always available
pub mod cache;
pub mod collection;
pub mod config;
pub mod crypt;
pub mod hash;
pub mod log;
pub mod path;
pub mod utils;

// HTTP facades — requires `http` feature
#[cfg(feature = "http")]
pub mod auth;
#[cfg(feature = "http")]
pub mod request;
#[cfg(feature = "http")]
pub mod response;
#[cfg(feature = "http")]
pub mod route;
#[cfg(feature = "http")]
pub mod session;

// Support facades — requires `support` feature
#[cfg(feature = "support")]
pub mod queue;
#[cfg(feature = "support")]
pub mod storage;

// ── Core re-exports ────────────────────────────────────────────────
pub use cache::Cache;
pub use collection::Collection;
pub use config::Config;
pub use crypt::Crypt;
pub use hash::Hash;
pub use log::Log;
pub use utils::{env, env_or, now};

// ── HTTP re-exports ────────────────────────────────────────────────
#[cfg(feature = "http")]
pub use auth::Auth;
#[cfg(feature = "http")]
pub use route::Route;
#[cfg(feature = "http")]
pub use session::Session;
#[cfg(feature = "http")]
pub use response::{abort, back, redirect};

// ── Support re-exports ───────────────────────────────────────────
#[cfg(feature = "support")]
pub use queue::Queue;
#[cfg(feature = "support")]
pub use storage::Storage;

// ── ApplicationExt ─────────────────────────────────────────────────
#[cfg(feature = "support")]
pub trait ApplicationExt: Sized {
    fn with_queue(self) -> Self;
}

#[cfg(feature = "support")]
impl ApplicationExt for ravel_core::app::Application {
    fn with_queue(self) -> Self {
        use ravel_support::queue::Queue;
        self.container().instance(Queue::memory());
        self
    }
}
