pub fn response() -> ravel_http::response::ResponseBuilder {
    ravel_http::response::response()
}

pub fn redirect(_url: &str) -> impl axum::response::IntoResponse {
    (axum::http::StatusCode::FOUND, [("location", _url.to_string())])
}

pub fn back() -> impl axum::response::IntoResponse {
    redirect("/")
}

pub fn abort(
    _status: u16,
    _message: impl Into<String>,
) -> ravel_http::error::RavelError {
    unimplemented!()
}
