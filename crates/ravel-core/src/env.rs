//! Environment variable support — load `.env` files and expose values
//! through the configuration system.
//!
//! Ravel loads `.env` files via [`dotenvy`] and makes values available:
//! - As `env()` calls inside TOML config strings (future).
//! - Through a dedicated `env.KEY` key in the config repo.
//! - Via the container as an [`EnvRepo`] singleton.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_core::env::EnvRepo;
//!
//! let env = EnvRepo::load(".env").unwrap();
//! assert_eq!(env.get("APP_NAME"), Some("MyApp".to_string()));
//! ```

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

/// A snapshot of environment variables loaded from `.env` files.
///
/// Internally merges file-defined variables with the real process
/// environment (real env takes precedence).
#[derive(Debug, Clone)]
pub struct EnvRepo {
    vars: HashMap<String, String>,
}

impl EnvRepo {
    /// Create an empty repo (no file loaded, process env only).
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }

    /// Load a `.env` file into the repo.
    ///
    /// Variables from the file are added to the internal map; existing
    /// real environment variables are **not** overwritten by the file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        let mut repo = Self::new();

        if path.is_file() {
            // dotenvy::from_filename_iter returns an iterator of (key, value) pairs.
            // It does NOT change the real process environment.
            let iter = dotenvy::from_filename_iter(path)?;

            for entry in iter {
                let (key, value) = entry?;
                repo.vars.insert(key, value);
            }
        }

        // Merge real environment on top (real env wins over file).
        for (key, value) in std::env::vars() {
            repo.vars.insert(key, value);
        }

        Ok(repo)
    }

    /// Load `.env` from the given directory (looks for `.env` inside it).
    pub fn load_from_dir(dir: impl AsRef<Path>) -> Result<Self> {
        Self::load(dir.as_ref().join(".env"))
    }

    /// Get an environment variable value.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(|s| s.as_str())
    }

    /// Get with a default fallback.
    pub fn get_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.get(key).unwrap_or(default)
    }

    /// Check whether a variable exists.
    pub fn has(&self, key: &str) -> bool {
        self.vars.contains_key(key)
    }

    /// Number of variables.
    pub fn len(&self) -> usize {
        self.vars.len()
    }

    /// No variables?
    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }
}

impl Default for EnvRepo {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_empty_repo() {
        let repo = EnvRepo::new();
        assert!(repo.is_empty());
        assert_eq!(repo.get("NOT_SET"), None);
    }

    #[test]
    fn test_load_dotenv_file() {
        let dir = std::env::temp_dir().join("ravel_env_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let env_path = dir.join(".env");
        let mut f = fs::File::create(&env_path).unwrap();
        writeln!(f, "APP_NAME=RavelApp").unwrap();
        writeln!(f, "APP_PORT=8080").unwrap();
        writeln!(f, "# a comment").unwrap();
        writeln!(f, "").unwrap();
        writeln!(f, "DB_HOST=localhost").unwrap();
        drop(f);

        let repo = EnvRepo::load(&env_path).unwrap();

        assert_eq!(repo.get("APP_NAME"), Some("RavelApp"));
        assert_eq!(repo.get("APP_PORT"), Some("8080"));
        assert_eq!(repo.get("DB_HOST"), Some("localhost"));
        assert!(!repo.has("# a comment")); // comments are skipped

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_real_env_overrides_file() {
        // Temporarily set a real env var
        unsafe { std::env::set_var("RAVEL_TEST_FOO", "from_real_env") };

        let dir = std::env::temp_dir().join("ravel_env_override_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let env_path = dir.join(".env");
        let mut f = fs::File::create(&env_path).unwrap();
        writeln!(f, "RAVEL_TEST_FOO=from_dotenv_file").unwrap();
        drop(f);

        let repo = EnvRepo::load(&env_path).unwrap();

        // Real env wins
        assert_eq!(repo.get("RAVEL_TEST_FOO"), Some("from_real_env"));

        unsafe { std::env::remove_var("RAVEL_TEST_FOO") };
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_missing_file_is_ok() {
        let repo = EnvRepo::load("/nonexistent/path/.env").unwrap();
        // Still contains real env vars from the process
        assert!(!repo.is_empty()); // because we merge real env
    }
}
