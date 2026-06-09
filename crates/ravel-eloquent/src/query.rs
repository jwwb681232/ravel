//! Type-safe query builder using `sea-query` instead of `EntityTrait`.
//!
//! Provides a fluent API similar to Laravel Eloquent but backed by
//! `sea-query`'s type-safe expression system instead of raw SQL strings.
//! No `EntityTrait` dependency means no duplicate entity struct is needed.

use sea_orm::{
    ColumnTrait, DatabaseConnection, Order, Statement, Value,
};
use sea_orm::sea_query::{self, Expr, Query, SelectStatement};
use sea_orm::sea_query::ExprTrait;
use sea_orm::ConnectionTrait;
use serde::de::DeserializeOwned;

use crate::error::{Result, RavelEloquentError};

// ── QueryBuilder ───────────────────────────────────────────────────────

/// Type-safe query builder using `sea-query` directly.
///
/// No entity type parameter needed — columns, table name and result types
/// are supplied when needed.
///
/// # Example
///
/// ```rust,ignore
/// use ravel_eloquent::QueryBuilder;
///
/// let users: Vec<User> = QueryBuilder::new("users", &["id", "name", "email"])
///     .filter(UserColumn::Name, "Alice")
///     .order_by_asc(UserColumn::Id)
///     .get(&db).await?;
/// ```
pub struct QueryBuilder {
    select: SelectStatement,
    columns: Vec<&'static str>,
}

impl QueryBuilder {
    /// Create a new query builder for `table_name` selecting `columns`.
    pub fn new(table_name: &'static str, columns: &'static [&'static str]) -> Self {
        let mut select = Query::select();
        for col in columns {
            let alias = sea_query::Alias::new(*col);
            let iden: sea_query::DynIden = alias.into();
            select.column(iden);
        }
        select.from(sea_query::Alias::new(table_name));
        Self {
            select,
            columns: columns.to_vec(),
        }
    }

    /// Escape hatch: consume self and return the underlying `SelectStatement`.
    pub fn into_select(self) -> SelectStatement {
        self.select
    }

    async fn run<T: DeserializeOwned>(self, db: &DatabaseConnection) -> Result<Vec<T>> {
        let backend = db.get_database_backend();
        let sql = Self::build_sql(&self.select, backend);
        let stmt = Statement::from_string(backend, sql);
        let rows = db
            .query_all_raw(stmt)
            .await
            .map_err(|e| RavelEloquentError::Database(e))?;
        rows.iter()
            .map(|row| row_to_model(row, &self.columns))
            .collect()
    }

    fn build_sql(select: &SelectStatement, backend: sea_orm::DbBackend) -> String {
        match backend {
            sea_orm::DbBackend::MySql => select.to_string(sea_query::MysqlQueryBuilder),
            sea_orm::DbBackend::Postgres => select.to_string(sea_query::PostgresQueryBuilder),
            sea_orm::DbBackend::Sqlite => select.to_string(sea_query::SqliteQueryBuilder),
            _ => {
                // Fallback: use SQLite builder as default for unknown backends
                select.to_string(sea_query::SqliteQueryBuilder)
            }
        }
    }
}

// ── WHERE conditions (type-safe column enum) ───────────────────────────

impl QueryBuilder {
    /// WHERE col = val
    pub fn filter(mut self, col: impl ColumnTrait, val: impl Into<Value>) -> Self {
        self.select.and_where(col.eq(val.into()));
        self
    }

    /// WHERE col > val
    pub fn filter_gt(mut self, col: impl ColumnTrait, val: impl Into<Value>) -> Self {
        self.select.and_where(col.gt(val.into()));
        self
    }

    /// WHERE col >= val
    pub fn filter_gte(mut self, col: impl ColumnTrait, val: impl Into<Value>) -> Self {
        self.select.and_where(col.gte(val.into()));
        self
    }

    /// WHERE col < val
    pub fn filter_lt(mut self, col: impl ColumnTrait, val: impl Into<Value>) -> Self {
        self.select.and_where(col.lt(val.into()));
        self
    }

    /// WHERE col <= val
    pub fn filter_lte(mut self, col: impl ColumnTrait, val: impl Into<Value>) -> Self {
        self.select.and_where(col.lte(val.into()));
        self
    }

    /// WHERE col != val
    pub fn filter_ne(mut self, col: impl ColumnTrait, val: impl Into<Value>) -> Self {
        self.select.and_where(col.ne(val.into()));
        self
    }

    /// WHERE col LIKE val
    pub fn filter_like(mut self, col: impl ColumnTrait, val: &str) -> Self {
        self.select.and_where(col.like(val));
        self
    }

    /// WHERE col IN (vals...)
    pub fn filter_in(mut self, col: impl ColumnTrait, vals: Vec<impl Into<Value>>) -> Self {
        let values: Vec<Value> = vals.into_iter().map(|v| v.into()).collect();
        self.select.and_where(col.is_in(values));
        self
    }

    /// WHERE col IS NULL
    pub fn filter_null(mut self, col: impl ColumnTrait) -> Self {
        self.select.and_where(col.is_null());
        self
    }

    /// WHERE col IS NOT NULL
    pub fn filter_not_null(mut self, col: impl ColumnTrait) -> Self {
        self.select.and_where(col.is_not_null());
        self
    }

    /// WHERE col BETWEEN low AND high
    pub fn filter_between(
        mut self,
        col: impl ColumnTrait,
        low: impl Into<Value>,
        high: impl Into<Value>,
    ) -> Self {
        self.select.and_where(col.between(low.into(), high.into()));
        self
    }
}

// ── String-based WHERE (convenience for macro-generated code) ──────────

impl QueryBuilder {
    /// Filter by column name (string) — convenience for macro-generated code.
    ///
    /// Prefer the type-safe [`filter`](Self::filter) methods that accept the
    /// Column enum directly.
    pub fn r#where(mut self, col: &str, val: impl Into<Value>) -> Self {
        use sea_orm::sea_query::{BinOper, ColumnRef, DynIden, SimpleExpr};

        let val_expr: SimpleExpr = sea_query::Value::from(val.into()).into();
        let col_ref: ColumnRef = DynIden::from(col.to_owned()).into();
        let condition = SimpleExpr::Binary(
            Box::new(SimpleExpr::Column(col_ref)),
            BinOper::Equal,
            Box::new(val_expr),
        );

        self.select.and_where(condition);
        self
    }
}

// ── ORDER BY, LIMIT, OFFSET ────────────────────────────────────────────

impl QueryBuilder {
    /// ORDER BY col (Direction)
    pub fn order_by(mut self, col: impl ColumnTrait, order: Order) -> Self {
        let (_, col_name) = col.as_column_ref();
        self.select.order_by(
            sea_query::ColumnRef::Column(col_name.into()),
            order,
        );
        self
    }

    /// ORDER BY col ASC
    pub fn order_by_asc(mut self, col: impl ColumnTrait) -> Self {
        let (_, col_name) = col.as_column_ref();
        self.select.order_by(
            sea_query::ColumnRef::Column(col_name.into()),
            sea_query::Order::Asc,
        );
        self
    }

    /// ORDER BY col DESC
    pub fn order_by_desc(mut self, col: impl ColumnTrait) -> Self {
        let (_, col_name) = col.as_column_ref();
        self.select.order_by(
            sea_query::ColumnRef::Column(col_name.into()),
            sea_query::Order::Desc,
        );
        self
    }

    /// LIMIT n
    pub fn limit(mut self, n: u64) -> Self {
        self.select.limit(n);
        self
    }

    /// OFFSET n
    pub fn offset(mut self, n: u64) -> Self {
        self.select.offset(n);
        self
    }
}

// ── Execution ──────────────────────────────────────────────────────────

impl QueryBuilder {
    /// Execute the query and return all matching rows.
    pub async fn get<T: DeserializeOwned>(self, db: &DatabaseConnection) -> Result<Vec<T>> {
        self.run(db).await
    }

    /// Execute the query and return the first matching row, if any.
    pub async fn first<T: DeserializeOwned>(mut self, db: &DatabaseConnection) -> Result<Option<T>> {
        self.select.limit(1);
        let mut items = self.run::<T>(db).await?;
        Ok(items.pop())
    }

    /// Execute COUNT and return the number of matching rows.
    pub async fn count(self, db: &DatabaseConnection) -> Result<u64> {
        let backend = db.get_database_backend();
        let mut count_select = Query::select();
        count_select
            .expr(Expr::col(sea_query::Asterisk).count())
            .from_subquery(self.select, sea_query::Alias::new("sub"));
        let sql = Self::build_sql(&count_select, backend);
        let stmt = Statement::from_string(backend, sql);
        let rows = db
            .query_all_raw(stmt)
            .await
            .map_err(|e| RavelEloquentError::Database(e))?;
        Ok(rows
            .first()
            .and_then(|r| r.try_get_by_index::<i64>(0).ok())
            .unwrap_or(0) as u64)
    }

    /// Check whether any matching rows exist.
    pub async fn exists(self, db: &DatabaseConnection) -> Result<bool> {
        self.count(db).await.map(|c| c > 0)
    }

    /// Paginate results.
    pub async fn paginate<T: DeserializeOwned>(
        self,
        db: &DatabaseConnection,
        page: u64,
        per_page: u64,
    ) -> Result<Page<T>> {
        let all_items = self.get::<T>(db).await?;
        let total = all_items.len() as u64;
        let offset = (page.saturating_sub(1)).saturating_mul(per_page) as usize;
        let items: Vec<_> = all_items
            .into_iter()
            .skip(offset)
            .take(per_page as usize)
            .collect();
        Ok(Page::new(items, total, Ord::max(page, 1), per_page))
    }
}

// ── Pagination ─────────────────────────────────────────────────────────

pub type Page<T> = ravel_db_core::pagination::Page<T>;

// ── SQL helpers (kept for relations.rs and ModelExt) ───────────────────

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
        Value::Char(Some(c)) => format!("'{}'", c),
        Value::Uuid(Some(u)) => format!("'{}'", u),
        Value::ChronoDate(Some(d)) => format!("'{}'", d),
        Value::ChronoDateTime(Some(dt)) => format!("'{}'", dt),
        Value::ChronoDateTimeUtc(Some(dt)) => format!("'{}'", dt),
        Value::Decimal(Some(d)) => format!("'{}'", d),
        Value::BigUnsigned(Some(u)) => u.to_string(),
        Value::Unsigned(Some(u)) => u.to_string(),
        _ => "NULL".to_string(),
    }
}

pub(crate) fn row_to_model<T: DeserializeOwned>(
    row: &sea_orm::QueryResult,
    cols: &[&str],
) -> Result<T> {
    let mut map = serde_json::Map::new();
    for (i, col) in cols.iter().enumerate() {
        let val = try_extract(row, i);
        map.insert(col.to_string(), val);
    }
    Ok(serde_json::from_value(serde_json::Value::Object(map))?)
}

fn try_extract(row: &sea_orm::QueryResult, index: usize) -> serde_json::Value {
    if let Ok(v) = row.try_get_by_index::<String>(index) {
        serde_json::Value::String(v)
    } else if let Ok(v) = row.try_get_by_index::<i64>(index) {
        serde_json::json!(v)
    } else if let Ok(v) = row.try_get_by_index::<i32>(index) {
        serde_json::json!(v)
    } else if let Ok(v) = row.try_get_by_index::<f64>(index) {
        serde_json::json!(v)
    } else if let Ok(v) = row.try_get_by_index::<bool>(index) {
        serde_json::json!(v)
    } else {
        serde_json::Value::Null
    }
}
