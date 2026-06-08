//! Log facade — re-exports tracing macros.
pub struct Log;

pub use ravel_core::log::{debug, error, info, trace, warn};
