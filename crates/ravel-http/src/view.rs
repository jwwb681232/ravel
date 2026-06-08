//! View rendering — Tera-based template engine integration.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_http::view::View;
//!
//! let view = View::new("templates/");
//! let html = view.render("welcome", &context! { name => "Alice" }).unwrap();
//! ```

use axum::response::{Html, IntoResponse, Response};
use std::path::Path;
use std::sync::Arc;
use tera::{Context, Tera};

/// Ravel's view renderer — wraps Tera with an ergonomic API.
pub struct View {
    tera: Arc<Tera>,
}

/// Helper to build a Tera `Context` from key-value pairs.
///
/// ```rust,ignore
/// use ravel_http::view::context;
///
/// let ctx = context! {
///     "title" => "Home",
///     "user" => &user,
/// };
/// ```
#[macro_export]
macro_rules! context {
    ($($key:expr => $val:expr),* $(,)?) => {{
        let mut ctx = ::tera::Context::new();
        $( ctx.insert($key, &$val); )*
        ctx
    }};
}

impl View {
    /// Create a new view renderer looking for templates in `templates_dir`.
    ///
    /// Templates are loaded once at construction time.  To pick up
    /// changes without restarting, create a new `View`.
    pub fn new(templates_dir: impl AsRef<Path>) -> Result<Self, tera::Error> {
        let pattern = templates_dir.as_ref().join("**/*");
        let tera = Tera::new(pattern.to_str().unwrap_or("templates/**/*"))?;
        Ok(Self {
            tera: Arc::new(tera),
        })
    }

    /// Render a template by name (relative to templates dir, without extension).
    ///
    /// ```rust,ignore
    /// let html = view.render("emails.welcome", &context! { "name" => "Alice" })?;
    /// ```
    pub fn render(&self, template: &str, ctx: &Context) -> Result<String, tera::Error> {
        let name = format!("{}.html", template);
        self.tera.render(&name, ctx)
    }

    /// Render a template and return an Axum `Html` response.
    ///
    /// ```rust,ignore
    /// async fn handler(view: &View) -> impl IntoResponse {
    ///     view.render_html("home", &context! { "title" => "Welcome" })
    /// }
    /// ```
    pub fn render_html(&self, template: &str, ctx: &Context) -> Result<Html<String>, tera::Error> {
        let html = self.render(template, ctx)?;
        Ok(Html(html))
    }

    /// Render and return an Axum `Response` (for use as `impl IntoResponse`).
    pub fn render_response(&self, template: &str, ctx: &Context) -> Result<Response, tera::Error> {
        let html = self.render(template, ctx)?;
        Ok(Html(html).into_response())
    }

    /// Add a raw template at runtime.
    pub fn add_raw_template(&mut self, name: &str, raw: &str) -> Result<(), tera::Error> {
        Arc::get_mut(&mut self.tera)
            .ok_or_else(|| tera::Error::msg("Tera is shared and cannot be mutated"))?
            .add_raw_template(name, raw)?;
        Ok(())
    }

    /// Return a reference to the inner Tera instance.
    pub fn tera(&self) -> &Tera {
        &self.tera
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_macro() {
        let ctx = context! {
            "foo" => "bar",
            "num" => 42,
        };
        // Just verify it compiles and doesn't panic
        let _ = ctx;
    }

    #[test]
    fn test_create_view_loads_templates() {
        let tmp = std::env::temp_dir().join("ravel_view_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        std::fs::write(tmp.join("hello.html"), "<h1>Hello, {{ name }}!</h1>").unwrap();

        let view = View::new(&tmp).unwrap();
        let ctx = context! { "name" => "Ravel" };
        let html = view.render("hello", &ctx).unwrap();

        assert_eq!(html, "<h1>Hello, Ravel!</h1>");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
