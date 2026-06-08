//! Configuration management — load `.toml` files from a directory
//! and access values via dot-notation keys.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_core::config::ConfigRepo;
//!
//! let repo = ConfigRepo::load_dir("config").unwrap();
//!
//! let name: String = repo.get("app.name").unwrap();
//! let port: u16 = repo.get("app.port").unwrap_or(3000);
//! ```

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// A repository of configuration values, loaded from TOML files.
///
/// Internally keys are stored in dot-notation (e.g. `app.name`, `database.port`).
#[derive(Debug, Clone)]
pub struct ConfigRepo {
    /// Flat map: "app.name" → `serde_json::Value`
    entries: BTreeMap<String, serde_json::Value>,
}

impl ConfigRepo {
    /// Create an empty repo.
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Load all `.toml` files from `dir` and merge them into a single repo.
    ///
    /// Files are loaded in alphabetical order; later files overwrite earlier
    /// ones on a per-key basis.
    pub fn load_dir(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        let mut repo = Self::new();

        if !dir.is_dir() {
            return Ok(repo); // no config directory → empty, no error
        }

        let mut paths: Vec<_> = fs::read_dir(dir)
            .with_context(|| format!("Cannot read config directory `{}`", dir.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "toml"))
            .collect();

        paths.sort();

        for path in &paths {
            let content = fs::read_to_string(path)
                .with_context(|| format!("Reading `{}`", path.display()))?;

            let table: toml::Table = toml::from_str(&content)
                .with_context(|| format!("Parsing `{}`", path.display()))?;

            repo.merge_toml_table(&table, "");
        }

        Ok(repo)
    }

    /// Load a single `.toml` file and merge it in.
    pub fn load_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let content =
            fs::read_to_string(path).with_context(|| format!("Reading `{}`", path.display()))?;
        let table: toml::Table =
            toml::from_str(&content).with_context(|| format!("Parsing `{}`", path.display()))?;
        self.merge_toml_table(&table, "");
        Ok(())
    }

    /// Set a value by key directly.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) {
        self.entries.insert(key.into(), value.into());
    }

    /// Get a typed value by key.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.entries
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Get a typed value by key, with a fallback default.
    pub fn get_or<T: DeserializeOwned>(&self, key: &str, default: T) -> T {
        self.get(key).unwrap_or(default)
    }

    /// Check whether a key exists.
    pub fn has(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    /// Number of keys.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// No keys?
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all (key, value) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &serde_json::Value)> {
        self.entries.iter()
    }

    /// Apply environment variable overrides.
    ///
    /// Variables with the `APP_` prefix are converted to dot-notation keys
    /// and override any values loaded from TOML files:
    ///
    /// - `APP_SERVER_HOST` → `server.host`
    /// - `APP_DATABASE_PORT` → `database.port`
    /// - `APP_NAME` → `name`
    /// - `APP_DEBUG` → `debug`
    ///
    /// Variables without the `APP_` prefix are silently ignored.
    pub fn apply_env_overrides(&mut self) {
        for (key, value) in std::env::vars() {
            if let Some(dotted) = Self::env_key_to_dotted(&key) {
                self.set(dotted, serde_json::Value::String(value));
            }
        }
    }

    /// Convert an env-var name to a dot-notation config key.
    ///
    /// Returns `None` if the variable does not start with `APP_`.
    ///
    /// ```
    /// # use ravel_core::config::ConfigRepo;
    /// assert_eq!(ConfigRepo::env_key_to_dotted("APP_SERVER_HOST"), Some("server.host".into()));
    /// assert_eq!(ConfigRepo::env_key_to_dotted("APP_NAME"), Some("name".into()));
    /// assert_eq!(ConfigRepo::env_key_to_dotted("PATH"), None);
    /// ```
    pub fn env_key_to_dotted(key: &str) -> Option<String> {
        let rest = key.strip_prefix("APP_")?;
        if rest.is_empty() {
            return None;
        }
        let parts: Vec<&str> = rest.splitn(2, '_').collect();
        let dotted = match parts.len() {
            1 => parts[0].to_lowercase(),
            _ => format!("{}.{}", parts[0].to_lowercase(), parts[1].to_lowercase()),
        };
        Some(dotted)
    }

    // ── internals ───────────────────────────────────────────────────

    /// Flatten a `toml::Table` into dot-notation keys and merge into `self`.
    fn merge_toml_table(&mut self, table: &toml::Table, prefix: &str) {
        for (key, value) in table {
            let full_key = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };

            match value {
                toml::Value::Table(inner) => {
                    self.merge_toml_table(inner, &full_key);
                }
                other => {
                    let json_val = toml_value_to_json(other);
                    self.entries.insert(full_key, json_val);
                }
            }
        }
    }
}

impl Default for ConfigRepo {
    fn default() -> Self {
        Self::new()
    }
}

// ── helpers ────────────────────────────────────────────────────────────

/// Convert a `toml::Value` (scalar/array) to `serde_json::Value`.
fn toml_value_to_json(v: &toml::Value) -> serde_json::Value {
    match v {
        toml::Value::String(s) => s.clone().into(),
        toml::Value::Integer(i) => (*i).into(),
        toml::Value::Float(f) => (*f).into(),
        toml::Value::Boolean(b) => (*b).into(),
        toml::Value::Datetime(d) => d.to_string().into(),
        toml::Value::Array(arr) => {
            let items: Vec<serde_json::Value> = arr.iter().map(toml_value_to_json).collect();
            serde_json::Value::Array(items)
        }
        toml::Value::Table(t) => {
            let map: serde_json::Map<String, serde_json::Value> = t
                .iter()
                .map(|(k, v)| (k.clone(), toml_value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn test_empty_repo() {
        let repo = ConfigRepo::new();
        assert!(repo.is_empty());
        assert_eq!(repo.get::<String>("nonexistent"), None);
    }

    #[test]
    fn test_set_and_get() {
        let mut repo = ConfigRepo::new();
        repo.set("app.name", "MyApp");
        repo.set("app.port", 8080u16);

        assert_eq!(repo.get::<String>("app.name").unwrap(), "MyApp");
        assert_eq!(repo.get::<u16>("app.port").unwrap(), 8080);
        assert_eq!(repo.get_or("app.debug", false), false);
    }

    #[test]
    fn test_load_dir() {
        let tmp = std::env::temp_dir().join("ravel_config_test");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        fs::write(
            tmp.join("app.toml"),
            r#"
[app]
name = "MyApp"
version = "1.0"

[database]
host = "localhost"
port = 5432
"#,
        )
        .unwrap();

        fs::write(
            tmp.join("overrides.toml"),
            r#"
[app]
port = 9000
"#,
        )
        .unwrap();

        let repo = ConfigRepo::load_dir(&tmp).unwrap();

        assert_eq!(repo.get::<String>("app.name").unwrap(), "MyApp");
        assert_eq!(repo.get::<String>("app.version").unwrap(), "1.0");
        assert_eq!(repo.get::<i64>("app.port").unwrap(), 9000); // overwritten
        assert_eq!(repo.get::<String>("database.host").unwrap(), "localhost");
        assert_eq!(repo.get::<i64>("database.port").unwrap(), 5432);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_missing_dir_is_ok() {
        let repo = ConfigRepo::load_dir("/nonexistent/path/ravel").unwrap();
        assert!(repo.is_empty());
    }

    #[test]
    fn test_array_value() {
        let mut repo = ConfigRepo::new();
        repo.set("app.allowed_hosts", vec!["a.com", "b.com"]);

        let hosts: Vec<String> = repo.get("app.allowed_hosts").unwrap();
        assert_eq!(hosts, vec!["a.com", "b.com"]);
    }

    // ── env_key_to_dotted ──────────────────────────────────────────

    #[test]
    fn test_env_key_simple() {
        assert_eq!(
            ConfigRepo::env_key_to_dotted("APP_NAME"),
            Some("name".into())
        );
    }

    #[test]
    fn test_env_key_nested() {
        assert_eq!(
            ConfigRepo::env_key_to_dotted("APP_SERVER_HOST"),
            Some("server.host".into())
        );
        assert_eq!(
            ConfigRepo::env_key_to_dotted("APP_DATABASE_PORT"),
            Some("database.port".into())
        );
    }

    #[test]
    fn test_env_key_three_segments() {
        // Only splits on first underscore after APP_
        assert_eq!(
            ConfigRepo::env_key_to_dotted("APP_DATABASE_MAX_CONNECTIONS"),
            Some("database.max_connections".into())
        );
    }

    #[test]
    fn test_env_key_non_app_ignored() {
        assert_eq!(ConfigRepo::env_key_to_dotted("PATH"), None);
        assert_eq!(ConfigRepo::env_key_to_dotted("HOME"), None);
        assert_eq!(ConfigRepo::env_key_to_dotted("SERVER_PORT"), None);
    }

    #[test]
    fn test_env_key_empty_prefix() {
        assert_eq!(ConfigRepo::env_key_to_dotted("APP_"), None);
    }

    // ── apply_env_overrides ────────────────────────────────────────

    #[test]
    fn test_apply_env_overrides_simple() {
        let _l = env_lock();
        unsafe {
            std::env::set_var("APP_NAME", "EnvApp");
            std::env::set_var("APP_PORT", "9999");
        }

        let mut repo = ConfigRepo::new();
        repo.set("name", "TomlApp");
        repo.apply_env_overrides();

        assert_eq!(repo.get::<String>("name").unwrap(), "EnvApp");
        assert_eq!(repo.get::<String>("port").unwrap(), "9999");

        unsafe {
            std::env::remove_var("APP_NAME");
            std::env::remove_var("APP_PORT");
        }
    }

    #[test]
    fn test_apply_env_overrides_nested() {
        let _l = env_lock();
        unsafe {
            std::env::set_var("APP_SERVER_HOST", "0.0.0.0");
        }

        let mut repo = ConfigRepo::new();
        repo.set("server.host", "127.0.0.1");
        repo.set("server.port", 3000u16);
        repo.apply_env_overrides();

        assert_eq!(repo.get::<String>("server.host").unwrap(), "0.0.0.0");
        // server.port should remain unchanged (not overridden by env)
        assert_eq!(repo.get::<u16>("server.port").unwrap(), 3000);

        unsafe {
            std::env::remove_var("APP_SERVER_HOST");
        }
    }

    #[test]
    fn test_apply_env_overrides_ignores_non_app() {
        let _l = env_lock();
        // PATH is always set — make sure it doesn't pollute config
        let mut repo = ConfigRepo::new();
        repo.set("server.host", "127.0.0.1");
        repo.apply_env_overrides();

        assert_eq!(repo.get::<String>("server.host").unwrap(), "127.0.0.1");
        // Non-APP_ vars should not create entries
        assert!(!repo.has("path"));
        assert!(!repo.has("home"));
    }
}
