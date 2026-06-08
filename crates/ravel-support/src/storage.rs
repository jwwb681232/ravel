//! File storage abstraction — pluggable disk drivers.
//!
//! # Built-in drivers
//!
//! | Driver | Description |
//! |--------|-------------|
//! | `local` | Reads/writes files on the local filesystem relative to `storage/`. |
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_support::storage::{Storage, LocalDisk};
//!
//! let disk = LocalDisk::new("storage");
//! Storage::disk(disk)
//!     .put("avatars/1.png", &image_bytes)
//!     .unwrap();
//!
//! let bytes = Storage::disk(disk).get("avatars/1.png").unwrap();
//! ```

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Uniform interface for file storage drivers.
pub trait Disk: Send + Sync {
    /// Store a file at `path`, creating parent directories as needed.
    fn put(&self, path: &str, contents: &[u8]) -> Result<()>;

    /// Read a file.
    fn get(&self, path: &str) -> Result<Vec<u8>>;

    /// Check whether a file exists.
    fn exists(&self, path: &str) -> bool;

    /// Delete a file.
    fn delete(&self, path: &str) -> Result<()>;

    /// Get the file size in bytes.
    fn size(&self, path: &str) -> Result<u64>;

    /// Return a publicly accessible URL (empty for local disk by default).
    fn url(&self, _path: &str) -> String {
        String::new()
    }
}

// ── Local disk ─────────────────────────────────────────────────────

/// Local filesystem driver, rooted at a base directory (typically `storage/`).
pub struct LocalDisk {
    root: PathBuf,
}

impl LocalDisk {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    fn full_path(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }
}

impl Disk for LocalDisk {
    fn put(&self, path: &str, contents: &[u8]) -> Result<()> {
        let full = self.full_path(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&full, contents)?;
        Ok(())
    }

    fn get(&self, path: &str) -> Result<Vec<u8>> {
        let full = self.full_path(path);
        fs::read(&full).with_context(|| format!("Reading {}", full.display()))
    }

    fn exists(&self, path: &str) -> bool {
        self.full_path(path).exists()
    }

    fn delete(&self, path: &str) -> Result<()> {
        let full = self.full_path(path);
        if full.is_file() {
            fs::remove_file(&full)?;
        }
        Ok(())
    }

    fn size(&self, path: &str) -> Result<u64> {
        let full = self.full_path(path);
        Ok(fs::metadata(&full)?.len())
    }
}

// ── Storage facade ─────────────────────────────────────────────────

/// The storage facade — mirrors Laravel's `Storage` helper.
pub struct Storage;

impl Storage {
    /// Store a file on the given disk.
    pub fn put(disk: &dyn Disk, path: &str, contents: &[u8]) -> Result<()> {
        disk.put(path, contents)
    }

    /// Read a file from the given disk.
    pub fn get(disk: &dyn Disk, path: &str) -> Result<Vec<u8>> {
        disk.get(path)
    }

    /// Check existence.
    pub fn exists(disk: &dyn Disk, path: &str) -> bool {
        disk.exists(path)
    }

    /// Delete a file.
    pub fn delete(disk: &dyn Disk, path: &str) -> Result<()> {
        disk.delete(path)
    }

    /// File size.
    pub fn size(disk: &dyn Disk, path: &str) -> Result<u64> {
        disk.size(path)
    }

    /// Public URL.
    pub fn url(disk: &dyn Disk, path: &str) -> String {
        disk.url(path)
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_put_get_delete() {
        let tmp = std::env::temp_dir().join("ravel_storage_test");
        let _ = fs::remove_dir_all(&tmp);

        let disk = LocalDisk::new(&tmp);

        disk.put("test/hello.txt", b"Hello, Ravel!").unwrap();
        assert!(disk.exists("test/hello.txt"));
        assert_eq!(disk.size("test/hello.txt").unwrap(), 13);

        let content = disk.get("test/hello.txt").unwrap();
        assert_eq!(content, b"Hello, Ravel!");

        disk.delete("test/hello.txt").unwrap();
        assert!(!disk.exists("test/hello.txt"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_storage_facade() {
        let tmp = std::env::temp_dir().join("ravel_storage_facade_test");
        let _ = fs::remove_dir_all(&tmp);

        let disk = LocalDisk::new(&tmp);
        Storage::put(&disk, "data.json", b"{}").unwrap();
        assert!(Storage::exists(&disk, "data.json"));

        let _ = fs::remove_dir_all(&tmp);
    }
}
