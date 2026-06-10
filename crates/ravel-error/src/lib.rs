use http::StatusCode;

/// Classification of an error for HTTP response mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    NotFound,
    BadRequest,
    Unauthorized,
    Forbidden,
    Validation,
    Conflict,
    TooManyRequests,
    Internal,
}

impl From<ErrorKind> for StatusCode {
    fn from(k: ErrorKind) -> Self {
        match k {
            ErrorKind::NotFound => StatusCode::NOT_FOUND,
            ErrorKind::BadRequest => StatusCode::BAD_REQUEST,
            ErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorKind::Forbidden => StatusCode::FORBIDDEN,
            ErrorKind::Validation => StatusCode::UNPROCESSABLE_ENTITY,
            ErrorKind::Conflict => StatusCode::CONFLICT,
            ErrorKind::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            ErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// Any error implementing this trait is automatically convertible
/// into an HTTP response by ravel-http's ErrorBridge.
pub trait HttpError: std::error::Error + Send + Sync + 'static {
    /// Which HTTP status code this error should produce.
    fn status_code(&self) -> StatusCode {
        self.kind().into()
    }

    /// The error's classification.
    fn kind(&self) -> ErrorKind {
        ErrorKind::Internal
    }
}
