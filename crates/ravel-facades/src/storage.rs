//! Storage facade — file storage abstraction.
use anyhow::Result;
use ravel_support::storage::{LocalDisk, Storage as StorageFacade};
use std::sync::OnceLock;

pub struct Storage;

impl Storage {
    fn disk() -> &'static LocalDisk {
        static DISK: OnceLock<LocalDisk> = OnceLock::new();
        DISK.get_or_init(|| LocalDisk::new("storage"))
    }

    pub fn put(path: &str, contents: &[u8]) -> Result<()> {
        StorageFacade::put(Self::disk(), path, contents)
    }

    pub fn get(path: &str) -> Result<Vec<u8>> {
        StorageFacade::get(Self::disk(), path)
    }

    pub fn exists(path: &str) -> bool {
        StorageFacade::exists(Self::disk(), path)
    }

    pub fn delete(path: &str) -> Result<()> {
        StorageFacade::delete(Self::disk(), path)
    }
}
