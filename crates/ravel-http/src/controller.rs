//! Controller trait — the building-block for request handlers.
//!
//! Every controller in a Ravel application implements this trait.
//! It integrates with the service container so that controllers
//! can receive dependencies via constructor injection.
//!
//! # Example
//!
//! ```rust,ignore
//! use ravel_http::controller::Controller;
//! use ravel_core::container::Container;
//!
//! struct UserController {
//!     // Dependencies resolved from the container
//! }
//!
//! impl Controller for UserController {
//!     fn boot(container: &Container) -> Self {
//!         // Resolve dependencies
//!         Self {}
//!     }
//! }
//! ```

use ravel_core::container::Container;

/// Every Ravel controller must implement this trait.
///
/// The [`boot`] factory receives the service container and should
/// return a fully-wired controller instance.
pub trait Controller: Send + Sync + Sized + 'static {
    /// Construct the controller, resolving dependencies from `container`.
    fn boot(container: &Container) -> Self;
}
