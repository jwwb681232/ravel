use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};

pub async fn handle(req: Request, next: Next) -> Response {
    // Pre-processing: runs before the handler
    tracing::info!("Cors: processing request {} {}", req.method(), req.uri());

    let response = next.run(req).await;

    // Post-processing: add CORS headers
    tracing::info!("Cors: response status {}", response.status());

    response
}
