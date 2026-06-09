//! Type-safe query builder wrapping SeaORM's `Select<E>`.
//!
//! Provides a fluent API similar to Laravel Eloquent but backed by
//! SeaORM 2.0's type-safe query system instead of raw SQL strings.

use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, Order,
    QueryFilter, QueryOrder, QuerySelect, Select, Value,
};
use serde::de::DeserializeOwned;

use crate::error::{Result, RavelEloquentError};

// ── QueryBuilder ───────────────────────────────────────────────────────

/// Type-safe query builder wrapping SeaORM's `Select<E>`.
///
/// `E` is the SeaORM entity type (not the model struct).
///
/// # Example
///
/// ```rust,ignore
/// use ravel_eloquent::QueryBuilder;
///
/// // Type-safe: use column enum directly
/// let users = QueryBuilder::<Entity>::new()
///     .filter(Column::Name, "Alice")
///     .order_by_asc(Column::Id)
///     .get(&db).await?;
///
/// // Escape hatch to raw SeaORM Select
/// let raw: Select<Entity> = query.into_select();
/// ```
pub struct QueryBuilder<E: EntityTrait> {
    select: Select<E>,
}

impl<E: EntityTrait> QueryBuilder<E> {
    /// Create a new query builder.
    pub fn new() -> Self {
        Self { select: E::find() }
    }

    /// Escape hatch: consume self and return the underlying SeaORM `Select<E>`.
    pub fn into_select(self) -> Select<E> {
        self.select
    }
}

impl<E: EntityTrait> Default for QueryBuilder<E> {
    fn default() -> Self {
        Self::new()
    }
}

// ── WHERE conditions (type-safe column enum) ───────────────────────────

impl<E: EntityTrait> QueryBuilder<E> {
    /// WHERE col = val
    pub fn filter(mut self, col: impl ColumnTrait, val: impl Into<Value>) -> Self {
        self.select = self.select.filter(col.eq(val.into()));
        self
    }

    /// WHERE col > val
    pub fn filter_gt(mut self, col: impl ColumnTrait, val: impl Into<Value>) -> Self {
        self.select = self.select.filter(col.gt(val.into()));
        self
    }

    /// WHERE col >= val
    pub fn filter_gte(mut self, col: impl ColumnTrait, val: impl Into<Value>) -> Self {
        self.select = self.select.filter(col.gte(val.into()));
        self
    }

    /// WHERE col < val
    pub fn filter_lt(mut self, col: impl ColumnTrait, val: impl Into<Value>) -> Self {
        self.select = self.select.filter(col.lt(val.into()));
        self
    }

    /// WHERE col <= val
    pub fn filter_lte(mut self, col: impl ColumnTrait, val: impl Into<Value>) -> Self {
        self.select = self.select.filter(col.lte(val.into()));
        self
    }

    /// WHERE col != val
    pub fn filter_ne(mut self, col: impl ColumnTrait, val: impl Into<Value>) -> Self {
        self.select = self.select.filter(col.ne(val.into()));
        self
    }

    /// WHERE col LIKE val
    pub fn filter_like(mut self, col: impl ColumnTrait, val: &str) -> Self {
        self.select = self.select.filter(col.like(val));
        self
    }

    /// WHERE col IN (vals...)
    pub fn filter_in(mut self, col: impl ColumnTrait, vals: Vec<impl Into<Value>>) -> Self {
        let values: Vec<Value> = vals.into_iter().map(|v| v.into()).collect();
        self.select = self.select.filter(col.is_in(values));
        self
    }

    /// WHERE col IS NULL
    pub fn filter_null(mut self, col: impl ColumnTrait) -> Self {
        self.select = self.select.filter(col.is_null());
        self
    }

    /// WHERE col IS NOT NULL
    pub fn filter_not_null(mut self, col: impl ColumnTrait) -> Self {
        self.select = self.select.filter(col.is_not_null());
        self
    }

    /// WHERE col BETWEEN low AND high
    pub fn filter_between(
        mut self,
        col: impl ColumnTrait,
        low: impl Into<Value>,
        high: impl Into<Value>,
    ) -> Self {
        self.select = self.select.filter(col.between(low.into(), high.into()));
        self
    }
}

// ── String-based WHERE (convenience for macro-generated code) ──────────

impl<E: EntityTrait> QueryBuilder<E> {
    /// Filter by column name (string) — convenience for macro-generated code.
    ///
    /// Prefer the type-safe [`filter`](Self::filter) methods that accept the
    /// Column enum directly.
    pub fn r#where(mut self, col: &str, val: impl Into<Value>) -> Self {
        use sea_orm::sea_query::{BinOper, ColumnRef, DynIden, SimpleExpr};

        // Convert sea_orm::Value -> sea_query::Value -> SimpleExpr
        let val_expr: SimpleExpr = sea_orm::sea_query::Value::from(val.into()).into();

        // Build column reference and equality expression
        let col_ref: ColumnRef = DynIden::from(col.to_owned()).into();
        let condition = SimpleExpr::Binary(
            Box::new(SimpleExpr::Column(col_ref)),
            BinOper::Equal,
            Box::new(val_expr),
        );

        self.select = self.select.filter(condition);
        self
    }
}

// ── ORDER BY, LIMIT, OFFSET ────────────────────────────────────────────

impl<E: EntityTrait> QueryBuilder<E> {
    /// ORDER BY col (Direction)
    pub fn order_by(mut self, col: impl ColumnTrait, order: Order) -> Self {
        self.select = self.select.order_by(col, order);
        self
    }

    /// ORDER BY col ASC
    pub fn order_by_asc(mut self, col: impl ColumnTrait) -> Self {
        self.select = self.select.order_by_asc(col);
        self
    }

    /// ORDER BY col DESC
    pub fn order_by_desc(mut self, col: impl ColumnTrait) -> Self {
        self.select = self.select.order_by_desc(col);
        self
    }

    /// LIMIT n
    pub fn limit(mut self, n: u64) -> Self {
        self.select = self.select.limit(n);
        self
    }

    /// OFFSET n
    pub fn offset(mut self, n: u64) -> Self {
        self.select = self.select.offset(n);
        self
    }
}

// ── JOIN ───────────────────────────────────────────────────────────────

impl<E: EntityTrait> QueryBuilder<E> {
    /// INNER JOIN with a related entity.
    pub fn inner_join<R: EntityTrait>(mut self, entity: R) -> Self
    where
        E: sea_orm::Related<R>,
    {
        self.select = self.select.inner_join(entity);
        self
    }

    /// LEFT JOIN with a related entity.
    pub fn left_join_related<R: EntityTrait>(mut self, entity: R) -> Self
    where
        E: sea_orm::Related<R>,
    {
        self.select = self.select.left_join(entity);
        self
    }
}

// ── Execution ──────────────────────────────────────────────────────────

impl<E: EntityTrait> QueryBuilder<E> {
    /// Execute the query and return all matching rows.
    pub async fn get(self, db: &DatabaseConnection) -> Result<Vec<E::Model>> {
        self.select
            .all(db)
            .await
            .map_err(|e| RavelEloquentError::Database(e))
    }

    /// Execute the query and return the first matching row, if any.
    pub async fn first(self, db: &DatabaseConnection) -> Result<Option<E::Model>> {
        self.select
            .one(db)
            .await
            .map_err(|e| RavelEloquentError::Database(e))
    }

    /// Execute COUNT and return the number of matching rows.
    pub async fn count(self, db: &DatabaseConnection) -> Result<u64> {
        let items = self
            .select
            .all(db)
            .await
            .map_err(|e| RavelEloquentError::Database(e))?;
        Ok(items.len() as u64)
    }

    /// Check whether any matching rows exist.
    pub async fn exists(self, db: &DatabaseConnection) -> Result<bool> {
        let items = self
            .select
            .all(db)
            .await
            .map_err(|e| RavelEloquentError::Database(e))?;
        Ok(!items.is_empty())
    }

    /// Paginate results.
    pub async fn paginate(
        self,
        db: &DatabaseConnection,
        page: u64,
        per_page: u64,
    ) -> Result<Page<E::Model>> {
        let all_items = self
            .select
            .all(db)
            .await
            .map_err(|e| RavelEloquentError::Database(e))?;
        let total = all_items.len() as u64;
        let offset = (page.saturating_sub(1)).saturating_mul(per_page) as usize;
        let items: Vec<_> = all_items
            .into_iter()
            .skip(offset)
            .take(per_page as usize)
            .collect();
        Ok(Page::new(items, total, page.max(1), per_page))
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
