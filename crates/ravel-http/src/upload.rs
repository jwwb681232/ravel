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
    /// The `dir` and `original_name` are sanitised to prevent path-traversal
    /// attacks: only the basename of the original filename is kept, parent-directory
    /// components are rejected, and the final resolved path is verified to reside
    /// inside [`storage_root`](Self::storage_root).
    ///
    /// Returns the full path where the file was written.
    pub async fn store(&self, dir: &str) -> std::io::Result<PathBuf> {
        // 1. Sanitize sub-directory — reject traversal attempts
        let safe_dir = sanitize_component(dir, "uploads")?;
        let dest_dir = self.storage_root.join(&safe_dir);
        tokio::fs::create_dir_all(&dest_dir).await?;

        // 2. Extract safe basename from original filename
        let raw_name = self.original_name.as_deref().unwrap_or("uploaded_file");
        let safe_name = sanitize_basename(raw_name);

        // 3. Build final path and verify containment within storage_root
        let path = dest_dir.join(&safe_name);

        // Canonicalize storage_root (must exist before we can canonicalize)
        let root = std::fs::canonicalize(&self.storage_root)
            .unwrap_or_else(|_| self.storage_root.clone());

        // Canonicalize the final path (parent dirs were created above)
        let resolved = std::fs::canonicalize(&path)
            .unwrap_or_else(|_| path.clone());

        if !resolved.starts_with(&root) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Path traversal detected",
            ));
        }

        tokio::fs::write(&resolved, &self.data).await?;
        Ok(resolved)
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

// ── Path sanitisation helpers ───────────────────────────────────────────

/// Extract the basename from a user-provided filename, stripping path
/// separators, control characters, and limiting length.
fn sanitize_basename(name: &str) -> String {
    let basename = std::path::Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("uploaded_file");

    basename
        .chars()
        .filter(|c| !c.is_control() && *c != '/' && *c != '\\' && *c != ':')
        .take(255)
        .collect::<String>()
}

/// Validate a user-provided path component, rejecting parent-directory
/// traversal and empty values.
fn sanitize_component(dir: &str, default: &str) -> std::io::Result<String> {
    let dir = dir.trim();
    if dir.is_empty() {
        return Ok(default.to_string());
    }
    let path = std::path::Path::new(dir);
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Invalid storage directory",
            ));
        }
    }
    Ok(dir.to_string())
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

    #[test]
    fn test_sanitize_basename_blocks_path_traversal() {
        let result = sanitize_basename("../../../etc/passwd");
        assert_eq!(result, "passwd");

        let result = sanitize_basename("..\\..\\windows\\system32");
        assert_eq!(result, "system32");

        let result = sanitize_basename("/absolute/path/file.txt");
        assert_eq!(result, "file.txt");

        let result = sanitize_basename("normal_file.jpg");
        assert_eq!(result, "normal_file.jpg");

        // Empty / missing
        let result = sanitize_basename("");
        assert_eq!(result, "uploaded_file");
    }

    #[test]
    fn test_sanitize_component_rejects_traversal() {
        assert!(sanitize_component("normal_dir", "default").is_ok());
        assert!(sanitize_component("sub/dir", "default").is_ok());
        assert!(sanitize_component("..", "default").is_err());
        assert!(sanitize_component("../etc", "default").is_err());
        assert_eq!(
            sanitize_component("", "default").unwrap(),
            "default"
        );
    }
}
