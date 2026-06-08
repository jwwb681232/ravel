//! ModelQuery — fluent Eloquent-style query builder.
//!
//! Builds query descriptions that can be executed via SeaORM.

use sea_orm::Value;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Fluent query builder with Laravel Eloquent-style chaining.
#[derive(Debug, Clone)]
pub struct ModelQuery<T> {
    pub(crate) table: String,
    pub(crate) wheres: Vec<(String, Value)>,
    pub(crate) orderings: Vec<(String, String)>,
    pub(crate) limit_val: Option<u64>,
    pub(crate) offset_val: Option<u64>,
    _marker: std::marker::PhantomData<T>,
}

impl<T: DeserializeOwned + Send + Sync + 'static> ModelQuery<T> {
    pub fn new() -> Self {
        Self {
            table: String::new(),
            wheres: Vec::new(),
            orderings: Vec::new(),
            limit_val: None,
            offset_val: None,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn table(mut self, name: impl Into<String>) -> Self {
        self.table = name.into();
        self
    }

    pub fn r#where(mut self, col: impl AsRef<str>, val: impl Into<Value>) -> Self {
        self.wheres.push((col.as_ref().to_string(), val.into()));
        self
    }

    pub fn order_by(mut self, col: impl AsRef<str>, dir: impl Into<String>) -> Self {
        self.orderings.push((col.as_ref().to_string(), dir.into()));
        self
    }

    pub fn limit(mut self, n: u64) -> Self {
        self.limit_val = Some(n);
        self
    }

    pub fn offset(mut self, n: u64) -> Self {
        self.offset_val = Some(n);
        self
    }

    /// Produce a SELECT SQL string.
    pub fn to_select_sql(&self) -> String {
        let mut sql = format!("SELECT * FROM \"{}\"", self.table);
        self.append_where(&mut sql);
        self.append_order(&mut sql);
        self.append_limit_offset(&mut sql);
        sql
    }

    /// Produce a COUNT SQL string.
    pub fn to_count_sql(&self) -> String {
        let mut sql = format!("SELECT COUNT(*) as count FROM \"{}\"", self.table);
        self.append_where(&mut sql);
        sql
    }

    /// Produce a DELETE SQL string.
    pub fn to_delete_sql(&self) -> String {
        let mut sql = format!("DELETE FROM \"{}\"", self.table);
        self.append_where(&mut sql);
        sql
    }

    fn append_where(&self, sql: &mut String) {
        if !self.wheres.is_empty() {
            let clauses: Vec<_> = self
                .wheres
                .iter()
                .map(|(col, val)| format!("\"{}\" = {}", col, quote_value(val)))
                .collect();
            *sql += &format!(" WHERE {}", clauses.join(" AND "));
        }
    }

    fn append_order(&self, sql: &mut String) {
        if !self.orderings.is_empty() {
            let clauses: Vec<_> = self
                .orderings
                .iter()
                .map(|(col, dir)| format!("\"{}\" {}", col, dir))
                .collect();
            *sql += &format!(" ORDER BY {}", clauses.join(", "));
        }
    }

    fn append_limit_offset(&self, sql: &mut String) {
        if let Some(n) = self.limit_val {
            *sql += &format!(" LIMIT {}", n);
        }
        if let Some(n) = self.offset_val {
            *sql += &format!(" OFFSET {}", n);
        }
    }
}

impl<T: DeserializeOwned + Send + Sync + 'static> Default for ModelQuery<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ── Pagination ─────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

impl<T> Page<T> {
    pub fn last_page(&self) -> u64 {
        self.total.div_ceil(self.per_page)
    }
    pub fn has_more(&self) -> bool {
        self.page < self.last_page()
    }
}

// ── ModelExt trait ─────────────────────────────────────────────────

#[async_trait::async_trait]
pub trait ModelExt: DeserializeOwned + Serialize + Send + Sync + 'static {
    async fn create(
        _data: serde_json::Value,
        _db: &sea_orm::DatabaseConnection,
    ) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        unimplemented!("ModelExt::create")
    }
    async fn update(
        &self,
        _data: serde_json::Value,
        _db: &sea_orm::DatabaseConnection,
    ) -> anyhow::Result<()> {
        unimplemented!("ModelExt::update")
    }
    async fn delete(&self, _db: &sea_orm::DatabaseConnection) -> anyhow::Result<()> {
        unimplemented!("ModelExt::delete")
    }
}

// ── Helpers ────────────────────────────────────────────────────────

pub(crate) fn quote_value(v: &Value) -> String {
    match v {
        Value::BigInt(Some(i)) => i.to_string(),
        Value::Int(Some(i)) => i.to_string(),
        Value::SmallInt(Some(i)) => i.to_string(),
        Value::TinyInt(Some(i)) => i.to_string(),
        Value::Float(Some(f)) => f.to_string(),
        Value::Double(Some(f)) => f.to_string(),
        Value::Bool(Some(b)) => (if *b { "TRUE" } else { "FALSE" }).to_string(),
        Value::String(Some(s)) => format!("'{}'", s.replace('\'', "''")),
        _ => "NULL".to_string(),
    }
}
