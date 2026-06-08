//! Password hashing via bcrypt.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_core::hash::Hash;
//!
//! let hashed = Hash::make("my-password")?;
//! assert!(Hash::check("my-password", &hashed)?);
//! assert!(!Hash::check("wrong-password", &hashed)?);
//! ```

use anyhow::{Context, Result};

/// Password hashing facade.
pub struct Hash;

impl Hash {
    /// Hash a plain-text password using bcrypt (cost 12).
    pub fn make(password: &str) -> Result<String> {
        bcrypt::hash(password, 12).context("Failed to hash password")
    }

    /// Verify a plain-text password against a bcrypt hash.
    pub fn check(password: &str, hash: &str) -> Result<bool> {
        bcrypt::verify(password, hash).context("Failed to verify password")
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_check() {
        let hashed = Hash::make("secret123").unwrap();
        assert!(Hash::check("secret123", &hashed).unwrap());
        assert!(!Hash::check("wrong", &hashed).unwrap());
    }

    #[test]
    fn test_different_passwords_different_hashes() {
        let a = Hash::make("a").unwrap();
        let b = Hash::make("b").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn test_verify_invalid_hash() {
        assert!(Hash::check("anything", "not-a-valid-hash").is_err());
    }
}
