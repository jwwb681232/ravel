//! Proc-macro for `#[derive(Model)]` — Eloquent-style ORM for Ravel.
//!
//! Generates SeaORM entity boilerplate and Eloquent-style query methods.
//!
//! ```rust,ignore
//! #[derive(ravel_eloquent::Model)]
//! #[model(table = "users")]
//! struct User {
//!     #[model(id)] id: i32,
//!     #[model(string, 255)] name: String,
//!     #[model(string, 255, unique)] email: String,
//!     #[model(hidden)] password: String,
//!     #[model(timestamps)] created_at: chrono::NaiveDateTime,
//! }
//! ```

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

mod attrs;
mod generator;

/// Derive macro that generates a SeaORM entity + Eloquent query API.
///
/// See [crate-level documentation](index.html) for usage.
#[proc_macro_derive(Model, attributes(model))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match generator::generate(&input) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
