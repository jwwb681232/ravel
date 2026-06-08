//! Hash facade — bcrypt password hashing.
use anyhow::Result;

pub struct Hash;

impl Hash {
    pub fn make(password: &str) -> Result<String> {
        ravel_core::hash::Hash::make(password)
    }

    pub fn check(password: &str, hash: &str) -> Result<bool> {
        ravel_core::hash::Hash::check(password, hash)
    }
}
