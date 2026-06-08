# Database: Getting Started

Ravel's database layer is built on **SeaORM** and supports PostgreSQL, MySQL, and SQLite. Connections are managed through `ConnectionManager` from `ravel-db-seaorm`, with lazy pooling and multi-connection support.

## Configuration

Create `config/database.toml` with one or more named connection sections:

```toml
[default]
driver = "postgres"
host = "localhost"
port = 5432
database = "myapp"
username = "user"
password = "secret"
max_connections = 20
min_connections = 5

[analytics]
driver = "mysql"
host = "analytics.internal"
port = 3306
database = "analytics"
username = "reader"
password = "readonly"
max_connections = 10
min_connections = 2
```

Supported drivers:

| Driver   | Connection string format |
|----------|--------------------------|
| postgres | `postgres://user:pass@host:5432/db` |
| mysql    | `mysql://user:pass@host:3306/db` |
| sqlite   | `sqlite:path/to/db.db?mode=rwc` |

## Connecting

Use `ConnectionManager` to load config and establish connections lazily:

```rust
use ravel_db_seaorm::connection::ConnectionManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load from config/database.toml
    let manager = ConnectionManager::from_config("config")?;

    // Connect on first use (lazy)
    let db = manager.connect("default").await?;

    // List configured connections
    for name in manager.config_names() {
        println!("Configured: {}", name);
    }

    Ok(())
}
```

The first call to `connect("name")` creates the connection pool. Subsequent calls return a cheap clone of the existing `DatabaseConnection` (it is `Arc`-based internally).

## Programmatic Registration

You can also register connections without a config file:

```rust
use ravel_db_seaorm::connection::{ConnectionManager, DatabaseConfig};

let mut manager = ConnectionManager::new();
manager.register("default", DatabaseConfig {
    driver: "sqlite".into(),
    host: String::new(),
    port: 0,
    database: "data/dev.db".into(),
    username: String::new(),
    password: String::new(),
    max_connections: 5,
    min_connections: 1,
});

let db = manager.connect("default").await?;
```

## Pool Settings

Each connection supports the following pool tuning parameters:

| Parameter       | Default | Description |
|-----------------|---------|-------------|
| `max_connections` | 20     | Maximum pool size |
| `min_connections` | 5      | Minimum idle connections |
| (hardcoded)     | —       | Connection timeout: 10s |
| (hardcoded)     | —       | Idle timeout: 300s |
| (hardcoded)     | —       | Acquire timeout: 30s |

## Service Provider Integration

Register the connection manager in your Application container:

```rust
use ravel_core::app::{Application, ServiceProvider};
use ravel_core::container::Container;
use ravel_db_seaorm::connection::ConnectionManager;

struct DatabaseServiceProvider;

impl ServiceProvider for DatabaseServiceProvider {
    fn register(&self, container: &Container) -> anyhow::Result<()> {
        let manager = ConnectionManager::from_config("config")?;
        container.instance(manager);
        Ok(())
    }

    fn name(&self) -> &str {
        "DatabaseServiceProvider"
    }
}
```

## Using in Tests

For tests, use SQLite for fast in-memory databases:

```rust
#[tokio::test]
async fn test_connection() {
    let mut manager = ConnectionManager::new();
    manager.register("test", DatabaseConfig {
        driver: "sqlite".into(),
        host: String::new(),
        port: 0,
        database: ":memory:".into(),
        username: String::new(),
        password: String::new(),
        max_connections: 1,
        min_connections: 1,
    });

    let db = manager.connect("test").await.unwrap();
    // Run queries against db ...
}
```
