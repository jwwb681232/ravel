//! Schema Builder — programmatic table creation API.
//!
//! Define database tables with a fluent Rust API instead of raw SQL.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_db_core::schema::Schema;
//!
//! Schema::create("users", |t| {
//!     t.id();
//!     t.string("name").nullable(false);
//!     t.string("email").unique();
//!     t.integer("age").default(0);
//!     t.timestamps();
//! });
//! // => "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, ...)"
//! ```

/// Column builder for defining columns fluently.
#[derive(Clone)]
pub struct ColumnDef {
    pub name: String,
    pub col_type: ColumnType,
    pub nullable: bool,
    pub unique: bool,
    pub primary_key: bool,
    pub auto_increment: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ColumnType {
    Integer,
    BigInt,
    String { length: usize },
    Text,
    Boolean,
    Float,
    Double,
    DateTime,
    Json,
    Uuid,
}

impl ColumnDef {
    pub fn new(name: impl Into<String>, col_type: ColumnType) -> Self {
        Self {
            name: name.into(),
            col_type,
            nullable: true,
            unique: false,
            primary_key: false,
            auto_increment: false,
            default_value: None,
        }
    }

    pub fn nullable(&mut self, v: bool) -> &mut Self {
        self.nullable = v;
        self
    }

    pub fn unique(&mut self) -> &mut Self {
        self.unique = true;
        self
    }

    pub fn primary(&mut self) -> &mut Self {
        self.primary_key = true;
        self.nullable = false;
        self
    }

    pub fn auto_increment(&mut self) -> &mut Self {
        self.auto_increment = true;
        self
    }

    pub fn default(&mut self, value: impl Into<String>) -> &mut Self {
        self.default_value = Some(value.into());
        self
    }
}

/// Fluent table builder.
pub struct Blueprint {
    table_name: String,
    columns: Vec<ColumnDef>,
    drop_if_exists: bool,
}

impl Blueprint {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            table_name: name.into(),
            columns: Vec::new(),
            drop_if_exists: false,
        }
    }

    /// Add DROP TABLE IF EXISTS before CREATE.
    pub fn drop_if_exists(mut self) -> Self {
        self.drop_if_exists = true;
        self
    }

    // ── Column helpers ──────────────────────────────────────────────

    pub fn id(&mut self) -> &mut ColumnDef {
        let mut col = ColumnDef::new("id", ColumnType::Integer);
        col.primary().auto_increment();
        self.columns.push(col);
        self.columns.last_mut().unwrap()
    }

    pub fn string(&mut self, name: impl Into<String>) -> &mut ColumnDef {
        let col = ColumnDef::new(name, ColumnType::String { length: 255 });
        self.columns.push(col);
        self.columns.last_mut().unwrap()
    }

    pub fn text(&mut self, name: impl Into<String>) -> &mut ColumnDef {
        let col = ColumnDef::new(name, ColumnType::Text);
        self.columns.push(col);
        self.columns.last_mut().unwrap()
    }

    pub fn integer(&mut self, name: impl Into<String>) -> &mut ColumnDef {
        let col = ColumnDef::new(name, ColumnType::Integer);
        self.columns.push(col);
        self.columns.last_mut().unwrap()
    }

    pub fn bigint(&mut self, name: impl Into<String>) -> &mut ColumnDef {
        let col = ColumnDef::new(name, ColumnType::BigInt);
        self.columns.push(col);
        self.columns.last_mut().unwrap()
    }

    pub fn boolean(&mut self, name: impl Into<String>) -> &mut ColumnDef {
        let mut col = ColumnDef::new(name, ColumnType::Boolean);
        col.default("false");
        self.columns.push(col);
        self.columns.last_mut().unwrap()
    }

    pub fn float(&mut self, name: impl Into<String>) -> &mut ColumnDef {
        let col = ColumnDef::new(name, ColumnType::Float);
        self.columns.push(col);
        self.columns.last_mut().unwrap()
    }

    pub fn datetime(&mut self, name: impl Into<String>) -> &mut ColumnDef {
        let col = ColumnDef::new(name, ColumnType::DateTime);
        self.columns.push(col);
        self.columns.last_mut().unwrap()
    }

    pub fn json(&mut self, name: impl Into<String>) -> &mut ColumnDef {
        let col = ColumnDef::new(name, ColumnType::Json);
        self.columns.push(col);
        self.columns.last_mut().unwrap()
    }

    pub fn uuid(&mut self, name: impl Into<String>) -> &mut ColumnDef {
        let col = ColumnDef::new(name, ColumnType::Uuid);
        self.columns.push(col);
        self.columns.last_mut().unwrap()
    }

    pub fn timestamps(&mut self) {
        let mut ca = ColumnDef::new("created_at", ColumnType::DateTime);
        ca.nullable(false).default("CURRENT_TIMESTAMP");
        self.columns.push(ca);
        let mut ua = ColumnDef::new("updated_at", ColumnType::DateTime);
        ua.nullable(false).default("CURRENT_TIMESTAMP");
        self.columns.push(ua);
    }

    // ── SQL generation ───────────────────────────────────────────────

    /// Generate the SQL CREATE TABLE statement.
    pub fn to_sql(&self, driver: DbDriver) -> String {
        let mut parts: Vec<String> = Vec::new();

        if self.drop_if_exists {
            parts.push(format!(
                "DROP TABLE IF EXISTS {}",
                self.quote(&self.table_name, driver)
            ));
        }

        let mut col_defs: Vec<String> = Vec::new();
        for col in &self.columns {
            col_defs.push(self.column_sql(col, driver));
        }

        parts.push(format!(
            "CREATE TABLE {} (\n  {}\n)",
            self.quote(&self.table_name, driver),
            col_defs.join(",\n  ")
        ));

        parts.join(";\n")
    }

    fn column_sql(&self, col: &ColumnDef, driver: DbDriver) -> String {
        let mut sql = format!("{} {}", self.quote(&col.name, driver), col.type_sql(driver));

        if !col.nullable {
            sql.push_str(" NOT NULL");
        }
        if col.unique {
            sql.push_str(" UNIQUE");
        }
        if col.primary_key {
            sql.push_str(" PRIMARY KEY");
        }
        if col.auto_increment {
            match driver {
                DbDriver::Postgres => sql.push_str(" GENERATED ALWAYS AS IDENTITY"),
                DbDriver::Mysql => sql.push_str(" AUTO_INCREMENT"),
                DbDriver::Sqlite => {} // SQLite handles INTEGER PRIMARY KEY implicitly
            }
        }
        if let Some(default) = &col.default_value {
            sql.push_str(&format!(" DEFAULT {default}"));
        }
        sql
    }

    fn quote(&self, name: &str, driver: DbDriver) -> String {
        match driver {
            DbDriver::Postgres | DbDriver::Sqlite => format!("\"{name}\""),
            DbDriver::Mysql => format!("`{name}`"),
        }
    }
}

impl ColumnDef {
    fn type_sql(&self, driver: DbDriver) -> &str {
        match (&self.col_type, driver) {
            (ColumnType::Integer, _) => "INTEGER",
            (ColumnType::BigInt, _) => "BIGINT",
            (ColumnType::String { length: _ }, _) => "VARCHAR(255)",
            (ColumnType::Text, _) => "TEXT",
            (ColumnType::Boolean, _) => "BOOLEAN",
            (ColumnType::Float, _) => "REAL",
            (ColumnType::Double, _) => "DOUBLE PRECISION",
            (ColumnType::DateTime, _) => "TIMESTAMP",
            (ColumnType::Json, DbDriver::Postgres) => "JSONB",
            (ColumnType::Json, _) => "TEXT",
            (ColumnType::Uuid, _) => "UUID",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DbDriver {
    Postgres,
    Mysql,
    Sqlite,
}

/// Schema builder entry point.
pub struct Schema;

impl Schema {
    /// Create a new table with the given blueprint.
    pub fn create(name: impl Into<String>, f: impl FnOnce(&mut Blueprint)) -> Blueprint {
        let mut bp = Blueprint::new(name);
        f(&mut bp);
        bp
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_users_table() {
        let bp = Schema::create("users", |t| {
            t.id();
            t.string("name").nullable(false);
            t.string("email").unique();
            t.integer("age").default("0");
            t.timestamps();
        });

        let sql = bp.to_sql(DbDriver::Sqlite);
        assert!(sql.contains("CREATE TABLE \"users\""));
        assert!(sql.contains("PRIMARY KEY"));
        assert!(sql.contains("UNIQUE"));
        assert!(sql.contains("DEFAULT 0"));
        assert!(sql.contains("created_at"));
        assert!(sql.contains("updated_at"));
    }

    #[test]
    fn test_create_posts_table() {
        let bp = Schema::create("posts", |t| {
            t.id();
            t.string("title");
            t.text("body");
            t.integer("user_id");
            t.boolean("published");
        });

        let sql = bp.to_sql(DbDriver::Postgres);
        assert!(sql.contains("CREATE TABLE \"posts\""));
        assert!(sql.contains("\"body\" TEXT"));
        assert!(sql.contains("GENERATED ALWAYS AS IDENTITY"));
    }

    #[test]
    fn test_drop_if_exists() {
        let bp = Schema::create("tmp", |t| {
            t.id();
        })
        .drop_if_exists();

        let sql = bp.to_sql(DbDriver::Sqlite);
        assert!(sql.contains("DROP TABLE IF EXISTS"));
    }

    #[test]
    fn test_column_types() {
        let bp = Schema::create("all_types", |t| {
            t.id();
            t.string("name");
            t.integer("count");
            t.bigint("total");
            t.boolean("active");
            t.float("score");
            t.datetime("published_at");
            t.text("description");
            t.json("metadata");
            t.uuid("external_id");
        });

        let sql = bp.to_sql(DbDriver::Postgres);
        assert!(sql.contains("VARCHAR(255)"));
        assert!(sql.contains("INTEGER"));
        assert!(sql.contains("BIGINT"));
        assert!(sql.contains("BOOLEAN"));
        assert!(sql.contains("JSONB"));
        assert!(sql.contains("UUID"));
    }
}
