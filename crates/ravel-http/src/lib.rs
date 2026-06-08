//! Ravel HTTP — Laravel-inspired HTTP layer built on Axum.
//!
//! Provides:
//! - Route DSL (attribute-like macros / Builder)
//! - Controller trait
//! - Middleware pipeline
//! - Request / Response wrappers

pub mod controller;
pub mod request;
pub mod response;
pub mod route;
