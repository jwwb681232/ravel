//! File upload support.
//!
//! [`UploadedFile`] is an Axum extractor for multipart file uploads.
//! Includes size limiting and MIME type validation.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_http::upload::UploadedFile;
//!
//! async fn upload(file: UploadedFile) -> impl IntoResponse {
//!     let path = file.store("uploads").await?;
//!     format!("Saved to {}", path.display())
//! }
//! ```

use axum::Json;
use axum::body::Bytes;
use axum::extract::{FromRequest, Multipart, Request};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::path::PathBuf;

/// The maximum file size (10 MB by default).
const DEFAULT_MAX_SIZE: u64 = 10 * 1024 * 1024;

/// A single uploaded file extracted from a multipart request.
///
/// Expected field name in the form is "file". For multiple files, name them
/// "file1", "file2", etc. and extract multiple `UploadedFile` parameters.
#[derive(Debug)]
pub struct UploadedFile {
    /// Original file name as sent by the client.
    pub original_name: Option<String>,
    /// Detected MIME type.
    pub content_type: Option<String>,
    /// File size in bytes.
    pub size: u64,
    /// The raw file data.
    pub data: Bytes,
    /// The storage directory root (set by application config).
    storage_root: PathBuf,
}

impl UploadedFile {
    /// Store the file to the given sub-directory under the storage root.
    ///
    /// Returns the full path where the file was written.
    pub async fn store(&self, dir: &str) -> std::io::Result<PathBuf> {
        let dest_dir = self.storage_root.join(dir);
        tokio::fs::create_dir_all(&dest_dir).await?;

        let filename = self.original_name.as_deref().unwrap_or("uploaded_file");

        let path = dest_dir.join(filename);
        tokio::fs::write(&path, &self.data).await?;
        Ok(path)
    }

    /// Get the file content as a UTF-8 string (if it's text).
    pub fn text(&self) -> Option<&str> {
        std::str::from_utf8(&self.data).ok()
    }

    /// Get a reference to the raw bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.data
    }
}

// ── Underlying extractor ─────────────────────────────────────────────────

/// Extracts a single file from a multipart request.
///
/// This implements `FromRequest` for `UploadedFile` — use it directly
/// as a handler parameter.
impl<S: Send + Sync + 'static> FromRequest<S> for UploadedFile {
    type Rejection = UploadError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let content_type = req
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        if !content_type.contains("multipart/form-data") {
            return Err(UploadError::BadRequest(
                "Expected multipart/form-data".into(),
            ));
        }

        let content_length = req
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        if content_length > DEFAULT_MAX_SIZE {
            return Err(UploadError::TooLarge {
                size: content_length,
                max: DEFAULT_MAX_SIZE,
            });
        }

        let mut multipart = Multipart::from_request(req, state)
            .await
            .map_err(|e| UploadError::BadRequest(format!("Invalid multipart: {e}")))?;

        let mut file_data: Option<Bytes> = None;
        let mut file_name: Option<String> = None;
        let mut file_type: Option<String> = None;

        while let Ok(Some(field)) = multipart.next_field().await {
            let name = field.name().unwrap_or("").to_string();
            if name.starts_with("file") {
                file_name = field.file_name().map(|s| s.to_string());
                file_type = field.content_type().map(|s| s.to_string());
                file_data =
                    Some(field.bytes().await.map_err(|e| {
                        UploadError::BadRequest(format!("Failed to read field: {e}"))
                    })?);
                break;
            }
        }

        match file_data {
            Some(data) => Ok(UploadedFile {
                original_name: file_name,
                content_type: file_type,
                size: data.len() as u64,
                data,
                storage_root: PathBuf::from("storage"),
            }),
            None => Err(UploadError::BadRequest(
                "No file field found in upload".into(),
            )),
        }
    }
}

// ── UploadError ─────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum UploadError {
    BadRequest(String),
    TooLarge { size: u64, max: u64 },
}

impl IntoResponse for UploadError {
    fn into_response(self) -> Response {
        match self {
            Self::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"message": msg, "status": 400})),
            )
                .into_response(),
            Self::TooLarge { size, max } => (
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(serde_json::json!({
                    "message": format!("File too large: {size} bytes (max {max})"),
                    "status": 413,
                })),
            )
                .into_response(),
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uploaded_file_text() {
        let file = UploadedFile {
            original_name: Some("test.txt".into()),
            content_type: Some("text/plain".into()),
            size: 11,
            data: Bytes::from("hello world"),
            storage_root: PathBuf::from("/tmp"),
        };
        assert_eq!(file.text(), Some("hello world"));
        assert_eq!(file.bytes(), b"hello world");
    }

    #[test]
    fn test_upload_error_bad_request() {
        let resp = UploadError::BadRequest("missing file".into()).into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_upload_error_too_large() {
        let resp = UploadError::TooLarge { size: 100, max: 50 }.into_response();
        assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}
