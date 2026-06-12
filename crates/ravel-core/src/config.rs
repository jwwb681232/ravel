//! Configuration management — load `.toml` files from a directory
//! and access values via dot-notation keys.
//!
//! # Configuration shape
//!
//! Each `.toml` file under `config/` is treated as a namespace equal to
//! its file stem. A file `config/database.toml` contributes keys under
//! the `database.*` namespace:
//!
//! ```toml
//! # config/database.toml
//! [default]
//! host = { from = "DATABASE_HOST", default = "127.0.0.1" }
//! port = { from = "DATABASE_PORT", default = 3306 }
//! ```
//!
//! Read with:
//!
//! ```rust,ignore
//! let host: String = Config::get("database.default.host", "0.0.0.0");
//! ```
//!
//! # Value forms
//!
//! A TOML value may be either:
//! - A plain literal (`"127.0.0.1"`, `3306`, `true`) — used as-is.
//! - An *env-bound* table `{ from = "ENV_VAR_NAME", default = <literal> }`:
//!   at startup the value is resolved from the process environment
//!   (which `dotenvy` has populated from `.env`). If the env var is
//!   missing, the `default` literal is used. If the `default` is also
//!   absent, startup fails with a precise error.
//!
//! This mirrors Laravel's `env('KEY', 'default')` style — the *file*
//! declares which environment variables it needs and what fallback to
//! use, while the *value* always comes from a single source at runtime.

use anyhow::{Context, Result, anyhow};
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

/// A repository of configuration values, loaded from TOML files.
///
/// Internally keys are stored in dot-notation (e.g. `app.name`,
/// `database.default.host`).
#[derive(Debug, Clone, Default)]
pub struct ConfigRepo {
    /// Resolved values: "app.name" → `serde_json::Value`.
    entries: BTreeMap<String, serde_json::Value>,
    /// Values that still need to be resolved against the environment
    /// (e.g. `host = { from = "DATABASE_HOST", default = "..." }`).
    /// Drained by [`resolve_env_overrides`](Self::resolve_env_overrides).
    pending: BTreeMap<String, PendingValue>,
}

impl ConfigRepo {
    /// Create an empty repo.
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            pending: BTreeMap::new(),
        }
    }

    /// Load all `.toml` files from `dir` and merge them into a single repo.
    ///
    /// Each file is treated as a namespace: its stem (file name without
    /// extension) is prepended to every key it contributes, separated by
    /// a dot. For example `config/database.toml` with `[default] host = ...`
    /// produces the key `database.default.host`.
    ///
    /// Files are loaded in alphabetical order; later files overwrite
    /// earlier ones on a per-key basis.
    ///
    /// Plain literal values are merged immediately. *Env-bound* values
    /// (written as `{ from = "...", default = ... }`) are stored as
    /// unresolved bindings; resolve them with
    /// [`resolve_env_overrides`](Self::resolve_env_overrides) after the
    /// environment has been loaded.
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

            // Use the file stem (e.g. "database.toml" → "database") as
            // the namespace prefix for every key inside.
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow!("Invalid config file name: {}", path.display()))?;
            repo.merge_toml_table(&table, stem);
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

    /// Resolve all pending env-bound values against the supplied
    /// environment map (typically the result of `dotenvy` + the real
    /// process environment).
    ///
    /// For each pending key:
    /// 1. If the named env var is present, parse it as the binding's
    ///    expected type (inferred from the `default` literal when
    ///    available, falling back to a string).
    /// 2. Otherwise, fall back to the `default` literal.
    /// 3. If the binding has no `default` and the env var is missing,
    ///    startup fails with a clear error pointing at the config key
    ///    and the env var name.
    pub fn resolve_env_overrides(&mut self, env: &HashMap<String, String>) -> Result<()> {
        let pending = std::mem::take(&mut self.pending);
        for (key, pv) in pending {
            let value = match pv {
                PendingValue::EnvBound(binding) => {
                    binding.resolve(env).with_context(|| {
                        format!(
                            "Failed to resolve config key `{key}` (env var `{}`)",
                            binding.from
                        )
                    })?
                }
            };
            self.entries.insert(key, value);
        }
        Ok(())
    }

    // ── internals ───────────────────────────────────────────────────

    /// Flatten a `toml::Table` into dot-notation keys and merge into `self`.
    ///
    /// `prefix` is the file-stem namespace (e.g. `"database"`); the empty
    /// string is only used for recursive sub-tables inside a single file.
    fn merge_toml_table(&mut self, table: &toml::Table, prefix: &str) {
        for (key, value) in table {
            let full_key = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };

            match value {
                toml::Value::Table(inner) => {
                    // Two cases for a sub-table:
                    //   - It looks like an env-bound binding
                    //     ({ from = "...", default = ... })
                    //   - It's a regular nested section
                    if let Some(binding) = EnvBinding::from_table(inner) {
                        self.pending
                            .insert(full_key, PendingValue::EnvBound(binding));
                    } else {
                        self.merge_toml_table(inner, &full_key);
                    }
                }
                other => {
                    let json_val = toml_value_to_json(other);
                    self.entries.insert(full_key, json_val);
                }
            }
        }
    }
}

// ── Env bindings ───────────────────────────────────────────────────────

/// A value written as `{ from = "ENV_VAR", default = <literal> }` in
/// a TOML config file. The `default` may be absent, in which case the
/// env var is required.
#[derive(Debug, Clone)]
struct EnvBinding {
    from: String,
    default: Option<serde_json::Value>,
}

/// A pending value that hasn't been resolved against the environment yet.
#[derive(Debug, Clone)]
enum PendingValue {
    EnvBound(EnvBinding),
}

impl EnvBinding {
    /// Try to recognise a sub-table as an env binding. Returns `None`
    /// if the table doesn't look like `{ from = ..., [default = ...] }`
    /// (in which case it should be treated as a regular nested section).
    fn from_table(t: &toml::Table) -> Option<Self> {
        // Must contain "from" and ONLY "from" + optional "default".
        if !t.contains_key("from") {
            return None;
        }
        let from = t.get("from")?.as_str()?.to_string();
        let default = t.get("default").map(toml_value_to_json);
        Some(Self { from, default })
    }

    /// Resolve this binding against the environment map.
    fn resolve(&self, env: &HashMap<String, String>) -> Result<serde_json::Value> {
        match env.get(&self.from) {
            Some(raw) => Ok(coerce_to_type(raw, self.default.as_ref())),
            None => self.default.clone().ok_or_else(|| {
                anyhow!(
                    "required env var `{}` is not set (no default provided in config)",
                    self.from
                )
            }),
        }
    }
}

/// Parse `raw` (a string from the environment) into a JSON value whose
/// shape matches the `default` literal. If `default` is `None`, the raw
/// string is returned as-is.
fn coerce_to_type(raw: &str, default: Option<&serde_json::Value>) -> serde_json::Value {
    let Some(target) = default else {
        return serde_json::Value::String(raw.to_string());
    };
    match target {
        serde_json::Value::Bool(_) => {
            // Accept "true"/"false" (case-insensitive), 1/0.
            match raw.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => serde_json::Value::Bool(true),
                "false" | "0" | "no" | "off" => serde_json::Value::Bool(false),
                _ => serde_json::Value::String(raw.to_string()),
            }
        }
        serde_json::Value::Number(_) => {
            // Try integer first, then float.
            if let Ok(parsed) = raw.trim().parse::<i64>() {
                serde_json::Value::Number(parsed.into())
            } else if let Ok(parsed) = raw.trim().parse::<f64>() {
                serde_json::Number::from_f64(parsed)
                    .map(serde_json::Value::Number)
                    .unwrap_or_else(|| serde_json::Value::String(raw.to_string()))
            } else {
                serde_json::Value::String(raw.to_string())
            }
        }
        serde_json::Value::String(_) => serde_json::Value::String(raw.to_string()),
        // For arrays/objects in default: fall back to the default's
        // shape — the env value can't be parsed structurally.
        other => other.clone(),
    }
}

// ── toml→json conversion (plain literals only) ────────────────────────

/// Convert a `toml::Value` (scalar/array/table) to `serde_json::Value`.
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

        // `app.toml` → namespace "app.*"; `overrides.toml` → "overrides.*"
        // but its `[app] port = 9000` key becomes "overrides.app.port",
        // NOT a top-level "app.port" override.
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

        assert_eq!(repo.get::<String>("app.app.name").unwrap(), "MyApp");
        assert_eq!(repo.get::<String>("app.app.version").unwrap(), "1.0");
        assert_eq!(repo.get::<String>("app.database.host").unwrap(), "localhost");
        assert_eq!(repo.get::<i64>("app.database.port").unwrap(), 5432);
        // overrides.toml has its own "app" namespace
        assert_eq!(repo.get::<i64>("overrides.app.port").unwrap(), 9000);

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

    // ── file-stem namespace ────────────────────────────────────────

    #[test]
    fn test_load_dir_uses_file_stem_as_namespace() {
        let tmp = std::env::temp_dir().join("ravel_config_ns_test");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        // `database.toml` → keys live under "database.*"
        fs::write(
            tmp.join("database.toml"),
            r#"
[default]
host = "localhost"
port = 5432
"#,
        )
        .unwrap();

        // `app.toml` → keys live under "app.*"
        fs::write(
            tmp.join("app.toml"),
            r#"
name = "Ravel"
"#,
        )
        .unwrap();

        let repo = ConfigRepo::load_dir(&tmp).unwrap();
        assert_eq!(
            repo.get::<String>("database.default.host").unwrap(),
            "localhost"
        );
        assert_eq!(repo.get::<i64>("database.default.port").unwrap(), 5432);
        assert_eq!(repo.get::<String>("app.name").unwrap(), "Ravel");

        let _ = fs::remove_dir_all(&tmp);
    }

    // ── env bindings ───────────────────────────────────────────────

    #[test]
    fn test_env_binding_resolves_from_env() {
        let mut repo = ConfigRepo::new();
        repo.pending.insert(
            "app.name".into(),
            PendingValue::EnvBound(EnvBinding {
                from: "APP_NAME".into(),
                default: Some(serde_json::Value::String("default".into())),
            }),
        );
        let env: HashMap<String, String> =
            [("APP_NAME".to_string(), "FromEnv".to_string())]
                .into_iter()
                .collect();
        repo.resolve_env_overrides(&env).unwrap();
        assert_eq!(repo.get::<String>("app.name").unwrap(), "FromEnv");
    }

    #[test]
    fn test_env_binding_uses_default_when_missing() {
        let mut repo = ConfigRepo::new();
        repo.pending.insert(
            "app.name".into(),
            PendingValue::EnvBound(EnvBinding {
                from: "APP_NAME".into(),
                default: Some(serde_json::Value::String("fallback".into())),
            }),
        );
        let env: HashMap<String, String> = HashMap::new();
        repo.resolve_env_overrides(&env).unwrap();
        assert_eq!(repo.get::<String>("app.name").unwrap(), "fallback");
    }

    #[test]
    fn test_env_binding_required_errors_when_missing() {
        let mut repo = ConfigRepo::new();
        repo.pending.insert(
            "app.key".into(),
            PendingValue::EnvBound(EnvBinding {
                from: "APP_KEY".into(),
                default: None,
            }),
        );
        let env: HashMap<String, String> = HashMap::new();
        let err = repo.resolve_env_overrides(&env).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("APP_KEY"), "error should mention env var: {msg}");
    }

    #[test]
    fn test_env_binding_coerces_types() {
        // bool
        let mut repo = ConfigRepo::new();
        repo.pending.insert(
            "app.debug".into(),
            PendingValue::EnvBound(EnvBinding {
                from: "APP_DEBUG".into(),
                default: Some(serde_json::Value::Bool(false)),
            }),
        );
        let env: HashMap<String, String> =
            [("APP_DEBUG".to_string(), "true".to_string())]
                .into_iter()
                .collect();
        repo.resolve_env_overrides(&env).unwrap();
        assert_eq!(repo.get::<bool>("app.debug").unwrap(), true);

        // integer
        let mut repo = ConfigRepo::new();
        repo.pending.insert(
            "app.port".into(),
            PendingValue::EnvBound(EnvBinding {
                from: "APP_PORT".into(),
                default: Some(serde_json::json!(3306)),
            }),
        );
        let env: HashMap<String, String> =
            [("APP_PORT".to_string(), "8080".to_string())]
                .into_iter()
                .collect();
        repo.resolve_env_overrides(&env).unwrap();
        assert_eq!(repo.get::<u16>("app.port").unwrap(), 8080);
    }

    #[test]
    fn test_env_binding_recognised_in_toml() {
        // The whole point: TOML writer writes { from = ..., default = ... }
        // and the loader recognises it as a binding, not a nested table.
        let tmp = std::env::temp_dir().join("ravel_config_binding_test");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        fs::write(
            tmp.join("database.toml"),
            r#"
[default]
host = { from = "DATABASE_HOST", default = "127.0.0.1" }
port = { from = "DATABASE_PORT", default = 3306 }
"#,
        )
        .unwrap();

        let env: HashMap<String, String> = [(
            "DATABASE_HOST".to_string(),
            "10.0.0.1".to_string(),
        )]
        .into_iter()
        .collect();

        let mut repo = ConfigRepo::load_dir(&tmp).unwrap();
        repo.resolve_env_overrides(&env).unwrap();

        // env wins where set
        assert_eq!(
            repo.get::<String>("database.default.host").unwrap(),
            "10.0.0.1"
        );
        // default used where env missing — and stays an integer
        assert_eq!(
            repo.get::<u16>("database.default.port").unwrap(),
            3306
        );

        let _ = fs::remove_dir_all(&tmp);
    }
}
