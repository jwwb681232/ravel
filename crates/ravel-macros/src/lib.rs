//! Procedural macros for the Ravel framework.
//!
//! ## Derives
//!
//! - `#[derive(Job)]` — implements the `Job` trait for a struct.

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, LitStr, parse_macro_input};

/// Derive macro that implements the `Job` trait.
///
/// Supports the following attributes:
/// - `#[job(name = "my_job")]` — override the default snake_case job name
/// - `#[job(queue = "high_priority")]` — set the queue name (default: "default")
/// - `#[job(max_attempts = 5)]` — set max retry attempts (default: 3)
///
/// # Example
///
/// ```rust,ignore
/// #[derive(Serialize, Deserialize, Job)]
/// #[job(name = "send_welcome", queue = "mail", max_attempts = 5)]
/// struct SendWelcomeEmail { user_id: u32 }
/// ```
#[proc_macro_derive(Job, attributes(job))]
pub fn derive_job(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Parse #[job(...)] attributes
    let mut job_name: Option<String> = None;
    let mut job_queue: Option<String> = None;
    let mut job_max_attempts: Option<u32> = None;

    for attr in &input.attrs {
        if !attr.path().is_ident("job") {
            continue;
        }
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                let s: LitStr = meta.value()?.parse()?;
                job_name = Some(s.value());
            } else if meta.path.is_ident("queue") {
                let s: LitStr = meta.value()?.parse()?;
                job_queue = Some(s.value());
            } else if meta.path.is_ident("max_attempts") {
                let s: LitStr = meta.value()?.parse()?;
                job_max_attempts = Some(s.value().parse().unwrap_or(3));
            }
            Ok(())
        });
    }

    let name_str = job_name.unwrap_or_else(|| camel_to_snake(&name.to_string()));
    let queue_str = job_queue.unwrap_or_else(|| "default".into());
    let max = job_max_attempts.unwrap_or(3);

    let expanded = quote! {
        #[async_trait::async_trait]
        impl ravel_support::queue::Job for #name {
            async fn handle(&self) -> anyhow::Result<()> {
                self.__ravel_job_handle().await
            }

            fn name() -> &'static str {
                #name_str
            }

            fn queue() -> &'static str {
                #queue_str
            }

            fn max_attempts() -> u32 {
                #max
            }
        }
    };

    TokenStream::from(expanded)
}

/// Convert `CamelCase` or `PascalCase` to `snake_case`.
fn camel_to_snake(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    let chars = s.chars().peekable();
    for c in chars {
        if c.is_uppercase() {
            if !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            out.push(c.to_lowercase().next().unwrap());
        } else {
            out.push(c);
        }
    }
    out
}
