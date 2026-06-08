//! Connection management traits.
//!
//! Defines [`ConnectionManager`] — implement this for each database backend
//! (SeaORM, sqlx, diesel, etc.).

use anyhow::Result;
use async_trait::async_trait;

/// Trait for database connection managers.
///
/// A connection manager loads configuration (typically from TOML files) and
/// provides named connections. Implementations can support multiple drivers
/// (PostgreSQL, MySQL, SQLite) and connection pooling.
///
/// # Type parameters
///
/// - `Conn`: the connection type (e.g. `sea_orm::DatabaseConnection`)
#[async_trait]
pub trait ConnectionManager: Send + Sync {
    /// The connection type managed by this implementation.
    type Conn: Send + Sync + Clone;

    /// Get (or create) a connection by its configuration name.
    async fn connect(&self, name: &str) -> Result<Self::Conn>;

    /// Return the names of all configured connections.
    fn config_names(&self) -> Vec<&str>;

    /// Check whether a named configuration exists.
    fn has_config(&self, name: &str) -> bool;
}
