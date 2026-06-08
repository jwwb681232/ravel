//! Ravel DB Core — database abstraction traits.
//!
//! This crate defines the traits that database implementations (such as
//! `ravel-db-seaorm`) must implement. Application code should depend on
//! this crate for the trait definitions; the CLI and runtime wire in the
//! concrete implementation.

pub mod connection;
pub mod migration;
pub mod pagination;
