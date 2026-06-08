//! Response helpers — response(), redirect(), back(), abort().

use axum::http::StatusCode;
use axum::response::IntoResponse;
use ravel_http::error::RavelError;

/// Create a new ResponseBuilder (delegates to ravel_http).
pub fn response() -> ravel_http::response::ResponseBuilder {
    ravel_http::response::response()
}

/// Redirect to a URL (302 Found).
pub fn redirect(url: &str) -> impl IntoResponse {
    (StatusCode::FOUND, [("location", url.to_string())])
}

/// Redirect back (302 Found, falls back to "/").
pub fn back() -> impl IntoResponse {
    redirect("/")
}

/// Abort with an HTTP error code.
/// Returns a RavelError (implements IntoResponse).
pub fn abort(status: u16, message: impl Into<String>) -> RavelError {
    let msg = message.into();
    match status {
        400 => RavelError::bad_request(msg),
        401 => RavelError::unauthorized(msg),
        403 => RavelError::forbidden(msg),
        404 => RavelError::not_found(msg),
        422 => RavelError::validation_error(
            std::collections::HashMap::from([("message".to_string(), vec![msg])])
        ),
        _ => RavelError::Internal(anyhow::anyhow!("{}", msg)),
    }
}
