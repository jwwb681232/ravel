use sea_orm::{ConnectionTrait, DatabaseConnection, ExprTrait, Statement, Value};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

use crate::error::{RavelEloquentError, Result};
use crate::relations::{RelationKind, RelationMeta};

/// Model metadata — auto-implemented by #[derive(Model)]
pub trait ModelMeta {
    /// JSON-safe public type (hidden fields excluded)
    type Public: Serialize;

    /// Column metadata enum type
    type Columns: Copy;

    /// Database table name
    fn table_name() -> &'static str;

    /// All database column names (excluding relation fields)
    fn columns() -> &'static [&'static str];

    /// Primary key column name
    fn id_column() -> &'static str;

    /// Non-hidden column names (for API serialization)
    fn public_columns() -> &'static [&'static str];

    /// Relationship metadata. Empty by default — overridden by #[derive(Model)].
    fn relations() -> &'static [RelationMeta] {
        &[]
    }

    /// Look up a relationship by its field name.
    fn get_relation(name: &str) -> Option<&'static RelationMeta> {
        for rel in Self::relations() {
            if rel.field_name == name {
                return Some(rel);
            }
        }
        None
    }
}

// ── ModelExt ─────────────────────────────────────────────────────────────

/// Static CRUD operations for models.
///
/// Implemented automatically by `#[derive(Model)]`.
/// Uses raw SQL internally since the entity type (`EntityTrait`) is not
/// available through `ModelMeta`. For type-safe queries, use `QueryBuilder`
/// with the entity type directly.
#[async_trait::async_trait]
pub trait ModelExt: ModelMeta + DeserializeOwned + Serialize + Send + Sync + 'static {
    /// Find a record by its primary key.
    async fn find(db: &DatabaseConnection, id: impl Into<Value> + Send) -> Result<Option<Self>> {
        let id_val: Value = id.into();
        let sql = format!(
            "SELECT * FROM \"{}\" WHERE \"{}\" = $1",
            Self::table_name(),
            Self::id_column()
        );
        let stmt = Statement::from_sql_and_values(db.get_database_backend(), &sql, [id_val]);
        let rows = db
            .query_all_raw(stmt)
            .await
            .map_err(RavelEloquentError::Database)?;
        if let Some(row) = rows.into_iter().next() {
            crate::query::row_to_model(&row, Self::columns()).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Find a record by its primary key, or return `RecordNotFound` error.
    async fn find_or_fail(db: &DatabaseConnection, id: impl Into<Value> + Send) -> Result<Self> {
        let id_val: Value = id.into();
        let id_str = format!("{:?}", id_val);
        Self::find(db, id_val)
            .await?
            .ok_or_else(|| RavelEloquentError::RecordNotFound {
                table: Self::table_name(),
                id: id_str,
            })
    }

    /// Fetch all records from the table.
    async fn all(db: &DatabaseConnection) -> Result<Vec<Self>> {
        let sql = format!("SELECT * FROM \"{}\"", Self::table_name());
        let rows = db
            .query_all_raw(Statement::from_string(db.get_database_backend(), sql))
            .await
            .map_err(RavelEloquentError::Database)?;
        rows.into_iter()
            .map(|row| crate::query::row_to_model(&row, Self::columns()))
            .collect()
    }

    /// Fetch all records with eager-loaded relations.
    async fn all_with(db: &DatabaseConnection, relations: &[&str]) -> Result<Vec<Self>> {
        let mut items = Self::all(db).await?;
        for rel in relations {
            Self::load_relation(rel, &mut items, db).await?;
        }
        Ok(items)
    }

    /// Eager-load a single relation onto already-fetched model instances.
    async fn load_relation(
        rel: &str,
        items: &mut Vec<Self>,
        db: &DatabaseConnection,
    ) -> Result<()> {
        let meta = Self::get_relation(rel).ok_or_else(|| {
            RavelEloquentError::Other(format!(
                "Relation '{}' not found on {}",
                rel,
                Self::table_name()
            ).into())
        })?;

        let columns = (meta.get_columns)();

        // Collect local-key values from the parent models.
        let ids: Vec<String> = items
            .iter()
            .map(|item| {
                let json = serde_json::to_value(item).unwrap();
                let id_val = json.get(meta.local_key).unwrap();
                match id_val {
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    _ => id_val.to_string(),
                }
            })
            .collect();

        if ids.is_empty() {
            return Ok(());
        }

        // Build a parameterised IN (...) query via sea-query.
        let values: Vec<sea_orm::Value> = ids
            .iter()
            .map(|id| {
                id.parse::<i64>()
                    .map(|i| sea_orm::Value::BigInt(Some(i)))
                    .unwrap_or_else(|_| sea_orm::Value::String(Some(id.clone())))
            })
            .collect();

        let sea_values: Vec<sea_orm::sea_query::Value> =
            values.into_iter().map(|v| v.into()).collect();

        let mut select = sea_orm::sea_query::Query::select();
        select.from(sea_orm::sea_query::Alias::new(meta.table_name));
        // Explicitly select all columns so the generated SQL has "SELECT *".
        for col in columns {
            select.column(sea_orm::sea_query::Alias::new(*col));
        }
        select.and_where(
            sea_orm::sea_query::Expr::col(sea_orm::sea_query::Alias::new(meta.foreign_key))
                .is_in(sea_values),
        );

        let sql = match db.get_database_backend() {
            sea_orm::DbBackend::MySql => {
                select.to_string(sea_orm::sea_query::MysqlQueryBuilder)
            }
            sea_orm::DbBackend::Postgres => {
                select.to_string(sea_orm::sea_query::PostgresQueryBuilder)
            }
            sea_orm::DbBackend::Sqlite => {
                select.to_string(sea_orm::sea_query::SqliteQueryBuilder)
            }
            _ => select.to_string(sea_orm::sea_query::SqliteQueryBuilder),
        };

        #[cfg(test)]
        eprintln!("[ravel-eloquent] eager-load SQL: {}", sql);

        let stmt = sea_orm::Statement::from_string(db.get_database_backend(), sql);
        let rows = db
            .query_all_raw(stmt)
            .await
            .map_err(RavelEloquentError::Database)?;

        // Group related rows by their foreign key.
        let mut grouped: HashMap<String, Vec<serde_json::Map<String, serde_json::Value>>> =
            HashMap::new();
        for row in rows {
            let mut map = serde_json::Map::new();
            for (i, col_name) in columns.iter().enumerate() {
                let val = crate::query::try_extract(&row, i);
                map.insert(col_name.to_string(), val);
            }
            let fk = match map.get(meta.foreign_key).unwrap() {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s.clone(),
                _ => map.get(meta.foreign_key).unwrap().to_string(),
            };
            grouped.entry(fk).or_default().push(map);
        }

        // Inject grouped data back into the parent JSON values.
        let mut json_values: Vec<serde_json::Value> =
            items.iter().map(|i| serde_json::to_value(i).unwrap()).collect();
        for val in &mut json_values {
            let id = match val.get(meta.local_key).unwrap() {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s.clone(),
                _ => val.get(meta.local_key).unwrap().to_string(),
            };
            if let Some(rel_items) = grouped.get(&id) {
                match meta.kind {
                    RelationKind::HasMany => {
                        val[rel] = serde_json::to_value(rel_items).unwrap();
                    }
                    RelationKind::HasOne | RelationKind::BelongsTo => {
                        val[rel] = serde_json::to_value(&rel_items.first()).unwrap();
                    }
                }
            }
        }

        // Re-deserialise into the parent model.
        let new_items: Vec<Self> = json_values
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect();
        *items = new_items;

        Ok(())
    }

    /// Create a new record from JSON data and return it.
    async fn create(data: serde_json::Value, db: &DatabaseConnection) -> Result<Self> {
        let cols: Vec<String> = Self::columns()
            .iter()
            .map(|c| format!("\"{}\"", c))
            .collect();
        let vals: Vec<String> = Self::columns()
            .iter()
            .map(|c| {
                data.get(c)
                    .map(quote_json_value)
                    // Use NULL for missing values — works across all backends
                    // including SQLite (triggers auto-increment for PK).
                    .unwrap_or_else(|| "NULL".into())
            })
            .collect();

        let sql = format!(
            "INSERT INTO \"{}\" ({}) VALUES ({}) RETURNING *",
            Self::table_name(),
            cols.join(", "),
            vals.join(", ")
        );

        let rows = db
            .query_all_raw(Statement::from_string(db.get_database_backend(), sql))
            .await
            .map_err(RavelEloquentError::Database)?;
        crate::query::row_to_model(
            &rows
                .into_iter()
                .next()
                .ok_or_else(|| RavelEloquentError::Other("INSERT returned no rows".into()))?,
            Self::columns(),
        )
    }

    /// Delete records by their primary key(s). Accepts a single id, a Vec, or an array.
    async fn destroy(db: &DatabaseConnection, ids: impl IntoIterator<Item = impl Into<Value>> + Send) -> Result<u64> {
        let quoted: Vec<String> = ids.into_iter().map(|id| crate::query::quote_value(&id.into())).collect();
        if quoted.is_empty() {
            return Ok(0);
        }
        let sql = format!(
            "DELETE FROM \"{}\" WHERE \"{}\" IN ({})",
            Self::table_name(),
            Self::id_column(),
            quoted.join(", "),
        );
        let result = db
            .execute_unprepared(&sql)
            .await
            .map_err(RavelEloquentError::Database)?;
        Ok(result.rows_affected())
    }
}

// ── ActiveModelExt ───────────────────────────────────────────────────────

/// Consumptive write operations — auto-implemented by #[derive(Model)].
///
/// Provides `save`, `insert`, `update`, `delete`, and `refresh` methods
/// that operate on model instances.
#[async_trait::async_trait]
pub trait ActiveModelExt: ModelExt {
    /// Persist the model: INSERT if id is 0/null, UPDATE otherwise.
    /// Returns the back-filled instance (with auto-generated id / timestamps).
    async fn save(self, db: &DatabaseConnection) -> Result<Self> {
        let data = serde_json::to_value(&self)?;
        let id_col = Self::id_column();
        let is_new = data
            .get(id_col)
            .map(|v| v.is_null() || v.as_i64() == Some(0))
            .unwrap_or(true);

        if is_new {
            let mut data = data;
            if let Some(obj) = data.as_object_mut() {
                obj.remove(id_col);
            }
            Self::create(data, db).await
        } else {
            let sets: Vec<_> = Self::columns()
                .iter()
                .filter(|c| **c != id_col)
                .filter_map(|c| {
                    data.get(c)
                        .map(|v| format!("\"{}\" = {}", c, quote_json_value(v)))
                })
                .collect();

            if sets.is_empty() {
                return Ok(self);
            }

            let id_val = data.get(id_col).unwrap();
            let id_str = quote_json_value(id_val);

            let sql = format!(
                "UPDATE \"{}\" SET {} WHERE \"{}\" = {}",
                Self::table_name(),
                sets.join(", "),
                id_col,
                id_str,
            );
            db.execute_unprepared(&sql)
                .await
                .map_err(RavelEloquentError::Database)?;

            let id_for_fetch = json_val_to_sea_value(id_val);
            Self::find(db, id_for_fetch)
                .await?
                .ok_or_else(|| RavelEloquentError::RecordNotFound {
                    table: Self::table_name(),
                    id: id_val.to_string(),
                })
        }
    }

    /// Force INSERT regardless of id value.
    async fn insert(self, db: &DatabaseConnection) -> Result<Self> {
        Self::create(serde_json::to_value(&self)?, db).await
    }

    /// Force UPDATE. Returns `UpdateWithoutId` error if id is 0/null.
    async fn update(self, db: &DatabaseConnection) -> Result<Self> {
        let data = serde_json::to_value(&self)?;
        let id_col = Self::id_column();
        let is_empty = data
            .get(id_col)
            .map(|v| v.is_null() || v.as_i64() == Some(0))
            .unwrap_or(true);

        if is_empty {
            return Err(RavelEloquentError::UpdateWithoutId);
        }
        self.save(db).await
    }

    /// DELETE the row. Consumes self.
    async fn delete(self, db: &DatabaseConnection) -> Result<()> {
        let data = serde_json::to_value(&self)?;
        let id_col = Self::id_column();
        let id_val = data
            .get(id_col)
            .ok_or_else(|| RavelEloquentError::Other("Cannot delete: missing id".into()))?;

        let sql = format!(
            "DELETE FROM \"{}\" WHERE \"{}\" = {}",
            Self::table_name(),
            id_col,
            quote_json_value(id_val),
        );
        db.execute_unprepared(&sql)
            .await
            .map_err(RavelEloquentError::Database)?;
        Ok(())
    }

    /// Re-fetch the record from the database by id.
    async fn refresh(self, db: &DatabaseConnection) -> Result<Self> {
        let data = serde_json::to_value(&self)?;
        let id_col = Self::id_column();
        let id_val = data
            .get(id_col)
            .ok_or_else(|| RavelEloquentError::Other("Cannot refresh: missing id".into()))?;

        let id_for_fetch = json_val_to_sea_value(id_val);
        Self::find(db, id_for_fetch)
            .await?
            .ok_or_else(|| RavelEloquentError::RecordNotFound {
                table: Self::table_name(),
                id: id_val.to_string(),
            })
    }
}

// ── Replicates ───────────────────────────────────────────────────────────

/// Clone a model instance — useful for creating a copy to persist.
pub trait Replicates: Clone + ModelMeta {
    /// Create a replica (deep copy) of this model.
    fn replicate(&self) -> Self {
        self.clone()
    }
}

// ── Serializes ───────────────────────────────────────────────────────────

/// JSON serialization helpers for models.
pub trait Serializes: ModelMeta + Serialize {
    /// Convert to the public-facing struct (hidden fields excluded).
    fn to_public(&self) -> Self::Public;

    /// Serialize self to a JSON `Value`.
    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// Serialize the public representation to a JSON `Value`.
    fn to_public_json(&self) -> serde_json::Value {
        serde_json::to_value(self.to_public()).unwrap_or(serde_json::Value::Null)
    }
}

// ── HasTimestamps ────────────────────────────────────────────────────────

/// Timestamp helper — touch `updated_at` on save.
///
/// Auto-implemented by `#[derive(Model)]` when timestamps are enabled.
#[async_trait::async_trait]
pub trait HasTimestamps: ActiveModelExt {
    /// Set `updated_at` to the current database time (`NOW()`).
    async fn touch(self, db: &DatabaseConnection) -> Result<Self> {
        let data = serde_json::to_value(&self)?;
        let id_col = Self::id_column();
        let id_val = data
            .get(id_col)
            .ok_or_else(|| RavelEloquentError::Other("Cannot touch: missing id".into()))?;

        let sql = format!(
            "UPDATE \"{}\" SET \"updated_at\" = NOW() WHERE \"{}\" = {}",
            Self::table_name(),
            id_col,
            quote_json_value(id_val),
        );
        db.execute_unprepared(&sql)
            .await
            .map_err(RavelEloquentError::Database)?;

        let id_for_fetch = json_val_to_sea_value(id_val);
        Self::find(db, id_for_fetch)
            .await?
            .ok_or_else(|| RavelEloquentError::RecordNotFound {
                table: Self::table_name(),
                id: id_val.to_string(),
            })
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn quote_json_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::Bool(b) => (if *b { "TRUE" } else { "FALSE" }).to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        _ => format!("'{}'", v),
    }
}

/// Convert a `serde_json::Value` to `sea_orm::Value` for use with `Self::find()`.
fn json_val_to_sea_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::BigInt(None),
        serde_json::Value::Bool(b) => Value::Bool(Some(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::BigInt(Some(i))
            } else if let Some(f) = n.as_f64() {
                Value::Double(Some(f))
            } else {
                Value::BigInt(None)
            }
        }
        serde_json::Value::String(s) => Value::String(Some(s.clone())),
        _ => Value::BigInt(None),
    }
}
