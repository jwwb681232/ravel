use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};

/// {{name}} — custom middleware.
///
/// Apply via: `Route::new().layer(axum::middleware::from_fn(handle))`
pub async fn handle(req: Request, next: Next) -> Response {
    // Pre-processing: runs before the handler
    tracing::info!("{{name}}: processing request {} {}", req.method(), req.uri());

    let response = next.run(req).await;

    // Post-processing: runs after the handler
    tracing::info!("{{name}}: response status {}", response.status());

    response
}
