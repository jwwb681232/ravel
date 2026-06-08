//! Ravel HTTP — Laravel-inspired HTTP layer built on Axum.
//!
//! Provides:
//! - Route DSL (attribute-like macros / Builder)
//! - Controller trait
//! - Middleware pipeline
//! - Request / Response wrappers
//! - View rendering (Tera)
//! - FormRequest validation
//! - Server helper

pub mod controller;
pub mod form_request;
pub mod middleware;
pub mod request;
pub mod response;
pub mod route;
pub mod server;
pub mod validation;
pub mod view;
