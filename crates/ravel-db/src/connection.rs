//! Connection Manager — multi-database connection handling.
//!
//! Reads configuration from `config/database.toml` and creates SeaORM
//! [`sea_orm::DatabaseConnection`] instances on demand.
//!
//! # config/database.toml format
//!
//! ```toml
//! [default]
//! driver = "postgres"    # postgres | mysql | sqlite
//! host = "localhost"
//! port = 5432
//! database = "myapp"
//! username = "user"
//! password = "pass"
//! max_connections = 20
//! min_connections = 5
//!
//! [slave]
//! driver = "sqlite"
//! database = "data/slave.db"
//! ```

use anyhow::{Context, Result};
use parking_lot::RwLock;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

// ── Connection config ──────────────────────────────────────────────

/// Deserialised database connection config.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DatabaseConfig {
    pub driver: String,

    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub database: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,

    // Pool settings
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
}

fn default_max_connections() -> u32 {
    20
}
fn default_min_connections() -> u32 {
    5
}

impl DatabaseConfig {
    /// Build the connection URL string.
    pub fn url(&self) -> String {
        match self.driver.as_str() {
            "postgres" | "postgresql" => format!(
                "postgres://{}:{}@{}:{}/{}",
                self.username, self.password, self.host, self.port, self.database
            ),
            "mysql" | "mariadb" => format!(
                "mysql://{}:{}@{}:{}/{}",
                self.username, self.password, self.host, self.port, self.database
            ),
            "sqlite" => format!("sqlite:{}?mode=rwc", self.database),
            other => other.to_string(), // raw URL
        }
    }

    /// Convert to SeaORM [`ConnectOptions`].
    pub fn to_connect_options(&self) -> ConnectOptions {
        let mut opt = ConnectOptions::new(self.url());
        opt.max_connections(self.max_connections)
            .min_connections(self.min_connections)
            .connect_timeout(Duration::from_secs(10))
            .idle_timeout(Duration::from_secs(300))
            .acquire_timeout(Duration::from_secs(30))
            .sqlx_logging(false);
        opt
    }
}

// ── Connection Manager ─────────────────────────────────────────────

/// Manages named database connections.
///
/// Thread-safe: can be shared across threads via `Arc<ConnectionManager>`.
/// Loads configuration from `config/database.toml` and lazily connects.
pub struct ConnectionManager {
    configs: HashMap<String, DatabaseConfig>,
    connections: RwLock<HashMap<String, DatabaseConnection>>,
}

impl ConnectionManager {
    /// Create an empty manager.
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            connections: RwLock::new(HashMap::new()),
        }
    }

    /// Load connection configs from a `database.toml` file or directory.
    ///
    /// If `path` is a directory, looks for `database.toml` inside it.
    pub fn from_config(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let file = if path.is_dir() {
            path.join("database.toml")
        } else {
            path.to_path_buf()
        };

        if !file.exists() {
            // No config file → empty manager (user adds connections programmatically)
            return Ok(Self::new());
        }

        let content = std::fs::read_to_string(&file)
            .with_context(|| format!("Reading {}", file.display()))?;

        let map: HashMap<String, DatabaseConfig> =
            toml::from_str(&content)
                .with_context(|| format!("Parsing {}", file.display()))?;

        Ok(Self {
            configs: map,
            connections: RwLock::new(HashMap::new()),
        })
    }

    /// Register a connection config programmatically.
    pub fn register(&mut self, name: impl Into<String>, config: DatabaseConfig) {
        self.configs.insert(name.into(), config);
    }

    /// Connect to a named database (or return a cached connection).
    ///
    /// Thread-safe: takes `&self` and lazily connects on first call.
    /// Returns a clone of the `DatabaseConnection` (cheap — it's Arc-based).
    pub async fn connect(&self, name: &str) -> Result<DatabaseConnection> {
        // Fast path: check if already connected
        {
            let conns = self.connections.read();
            if let Some(db) = conns.get(name) {
                return Ok(db.clone());
            }
        }

        // Slow path: connect and cache
        let config = self
            .configs
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("No database config for '{name}'"))?;

        let db = Database::connect(config.to_connect_options())
            .await
            .with_context(|| format!("Connecting to database '{name}'"))?;

        self.connections
            .write()
            .insert(name.to_string(), db.clone());

        Ok(db)
    }

    /// Return all config names.
    pub fn config_names(&self) -> Vec<&String> {
        self.configs.keys().collect()
    }

    /// Check whether a config exists.
    pub fn has_config(&self, name: &str) -> bool {
        self.configs.contains_key(name)
    }

    /// Check whether a connection is established.
    pub fn is_connected(&self, name: &str) -> bool {
        self.connections.read().contains_key(name)
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_generation() {
        let cfg = DatabaseConfig {
            driver: "postgres".into(),
            host: "localhost".into(),
            port: 5432,
            database: "mydb".into(),
            username: "user".into(),
            password: "pass".into(),
            max_connections: 10,
            min_connections: 2,
        };
        assert_eq!(cfg.url(), "postgres://user:pass@localhost:5432/mydb");
    }

    #[test]
    fn test_sqlite_url() {
        let cfg = DatabaseConfig {
            driver: "sqlite".into(),
            host: String::new(),
            port: 0,
            database: "data/app.db".into(),
            username: String::new(),
            password: String::new(),
            max_connections: 1,
            min_connections: 1,
        };
        assert_eq!(cfg.url(), "sqlite:data/app.db?mode=rwc");
    }

    #[test]
    fn test_load_from_toml() {
        let tmp = std::env::temp_dir().join("ravel_db_config_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        std::fs::write(
            tmp.join("database.toml"),
            r#"
[default]
driver = "postgres"
host = "localhost"
port = 5432
database = "myapp"
username = "user"
password = "secret"

[cache]
driver = "sqlite"
database = "cache.db"
"#,
        )
        .unwrap();

        let manager = ConnectionManager::from_config(&tmp).unwrap();
        assert!(manager.has_config("default"));
        assert!(manager.has_config("cache"));
        assert!(!manager.has_config("nonexistent"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
