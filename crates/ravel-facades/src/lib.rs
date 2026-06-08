//! Laravel-style static facades for the Ravel framework.
//!
//! All facades are usable after Application::boot() has completed.

pub mod cache;
pub mod collection;
pub mod config;
pub mod crypt;
pub mod hash;
pub mod log;
pub mod path;
pub mod queue;
pub mod request;
pub mod response;
pub mod route;
pub mod session;
pub mod auth;
pub mod storage;
pub mod utils;

/// Extension trait for Application to register services from ravel-support.
pub trait ApplicationExt: Sized {
    fn with_queue(self) -> Self;
}

impl ApplicationExt for ravel_core::app::Application {
    fn with_queue(self) -> Self {
        use ravel_support::queue::Queue;
        self.container().instance(Queue::memory());
        self
    }
}
