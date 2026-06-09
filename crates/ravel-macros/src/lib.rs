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

// ── #[ravel::test] ─────────────────────────────────────────────────────

/// Attribute macro that replaces `#[test]` with automatic Ravel application
/// lifecycle management.
///
/// Each test gets a fresh `Application::new().boot()` wrapped in
/// `with_app()` — no manual resets or global locks needed.
/// or `BOOT_LOCK` needed.
///
/// # Sync test
///
/// ```rust,ignore
/// #[ravel::test]
/// fn test_config() {
///     let val: Option<String> = Config::get("key");
///     assert_eq!(val, None);
/// }
/// ```
///
/// # Async test
///
/// ```rust,ignore
/// #[ravel::test]
/// async fn test_request() {
///     let client = TestClient::new(Route::build());
///     let resp = client.get("/").await;
///     resp.assert_ok();
/// }
/// ```
///
/// # Custom application
///
/// ```rust,ignore
/// #[ravel::test(app = my_app)]
/// fn test_cache() {
///     Cache::put("k", "v", None);
/// }
///
/// fn my_app() -> ravel_core::app::Application {
///     ravel_core::app::Application::new().with_cache()
/// }
/// ```
struct TestMacroArgs {
    app_factory: Option<proc_macro2::TokenStream>,
}

impl syn::parse::Parse for TestMacroArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut app_factory = None;

        // Parse comma-separated meta items: `app = "path"`
        let metas =
            syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated(input)?;
        for meta in &metas {
            if let syn::Meta::NameValue(nv) = meta {
                if nv.path.is_ident("app") {
                    if let syn::Expr::Lit(expr_lit) = &nv.value {
                        if let syn::Lit::Str(lit) = &expr_lit.lit {
                            let path: syn::Path =
                                lit.parse().expect("invalid path in `app = \"...\"`");
                            app_factory = Some(quote! { #path() });
                        }
                    }
                }
            }
        }

        Ok(TestMacroArgs { app_factory })
    }
}

#[proc_macro_attribute]
pub fn test(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::ItemFn);
    let args = parse_macro_input!(attr as TestMacroArgs);

    let fn_name = &input.sig.ident;
    let fn_block = &input.block;
    let fn_vis = &input.vis;
    let fn_attrs: Vec<_> = input
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("test"))
        .collect();

    let app_expr = args.app_factory.unwrap_or_else(|| {
        quote! { ravel_core::app::Application::new() }
    });

    // All tests become async — task-local APP requires a tokio context.
    let output = quote! {
        #fn_vis #[tokio::test]
        #(#fn_attrs)*
        async fn #fn_name() {
            ravel_facades::Route::reset();
            let app = #app_expr.boot().expect("Failed to boot Ravel application");
            ravel_core::app::with_app(app, async move {
                #fn_block
            }).await;
        }
    };

    output.into()
}
