//! Type-safe query builder using `sea-query` instead of `EntityTrait`.
//!
//! Provides a fluent API similar to Laravel Eloquent but backed by
//! `sea-query`'s type-safe expression system instead of raw SQL strings.
//! No `EntityTrait` dependency means no duplicate entity struct is needed.

use sea_orm::ConnectionTrait;
use sea_orm::sea_query::ExprTrait;
use sea_orm::sea_query::{self, Expr, Query, SelectStatement};
use sea_orm::{ColumnTrait, DatabaseConnection, Order, Statement, Value};
use serde::de::DeserializeOwned;

use crate::error::{RavelEloquentError, Result};
use crate::model_traits::{ModelExt, ModelMeta};
use crate::relations::RelationKind;

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
///     .where_eq(UserColumn::Name, "Alice")
///     .order_by_asc(UserColumn::Id)
///     .get(&db).await?;
/// ```
pub struct QueryBuilder {
    select: SelectStatement,
    columns: Vec<&'static str>,
    eager_loads: Vec<String>,
}

impl QueryBuilder {
    /// Create a new query builder for `table_name` selecting `columns`.
    pub fn new(table_name: &'static str, columns: &'static [&'static str]) -> Self {
        Self::new_inner(table_name, columns, false)
    }

    /// Create a query builder that includes soft-deleted records.
    pub fn new_with_trashed(table_name: &'static str, columns: &'static [&'static str]) -> Self {
        Self::new_inner(table_name, columns, true)
    }

    /// Create a query builder that only fetches soft-deleted records.
    pub fn new_only_trashed(
        table_name: &'static str,
        columns: &'static [&'static str],
        soft_delete_col: &'static str,
    ) -> Self {
        let mut select = Query::select();
        for col in columns {
            let alias = sea_query::Alias::new(*col);
            let iden: sea_query::DynIden = alias.into();
            select.column(iden);
        }
        select.from(sea_query::Alias::new(table_name));
        select.and_where(
            sea_query::Expr::col(sea_query::Alias::new(soft_delete_col)).is_not_null(),
        );
        Self {
            select,
            columns: columns.to_vec(),
            eager_loads: Vec::new(),
        }
    }

    fn new_inner(
        table_name: &'static str,
        columns: &'static [&'static str],
        with_trashed: bool,
    ) -> Self {
        let mut select = Query::select();
        for col in columns {
            let alias = sea_query::Alias::new(*col);
            let iden: sea_query::DynIden = alias.into();
            select.column(iden);
        }
        select.from(sea_query::Alias::new(table_name));
        let builder = Self {
            select,
            columns: columns.to_vec(),
            eager_loads: Vec::new(),
        };
        // If NOT with_trashed, check for soft delete column and filter if present
        if !with_trashed {
            // We can't access ModelMeta here directly, so the caller handles this.
            // The `query()` generated method will inject the soft_delete filter.
        }
        builder
    }

    /// Inject a soft-delete WHERE clause into this query.
    ///
    /// Called by `#[derive(Model)]` when `soft_deletes` is enabled.
    pub fn with_soft_delete_filter(mut self, col: &'static str) -> Self {
        self.select
            .and_where(sea_query::Expr::col(sea_query::Alias::new(col)).is_null());
        self
    }

    /// Escape hatch: consume self and return the underlying `SelectStatement`.
    pub fn into_select(self) -> SelectStatement {
        self.select
    }

    /// Eager-load a relation after the main query is executed.
    pub fn with(mut self, relation: &str) -> Self {
        self.eager_loads.push(relation.to_string());
        self
    }

    async fn run<T: DeserializeOwned>(self, db: &DatabaseConnection) -> Result<Vec<T>> {
        let backend = db.get_database_backend();
        let sql = Self::build_sql(&self.select, backend);
        let stmt = Statement::from_string(backend, sql);
        let rows = db
            .query_all_raw(stmt)
            .await
            .map_err(RavelEloquentError::Database)?;
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
    pub fn where_eq(mut self, col: impl ColumnTrait, val: impl Into<Value>) -> Self {
        self.select.and_where(col.eq(val.into()));
        self
    }

    /// WHERE col > val
    pub fn where_gt(mut self, col: impl ColumnTrait, val: impl Into<Value>) -> Self {
        self.select.and_where(col.gt(val.into()));
        self
    }

    /// WHERE col >= val
    pub fn where_gte(mut self, col: impl ColumnTrait, val: impl Into<Value>) -> Self {
        self.select.and_where(col.gte(val.into()));
        self
    }

    /// WHERE col < val
    pub fn where_lt(mut self, col: impl ColumnTrait, val: impl Into<Value>) -> Self {
        self.select.and_where(col.lt(val.into()));
        self
    }

    /// WHERE col <= val
    pub fn where_lte(mut self, col: impl ColumnTrait, val: impl Into<Value>) -> Self {
        self.select.and_where(col.lte(val.into()));
        self
    }

    /// WHERE col != val
    pub fn where_ne(mut self, col: impl ColumnTrait, val: impl Into<Value>) -> Self {
        self.select.and_where(col.ne(val.into()));
        self
    }

    /// WHERE col LIKE val
    pub fn where_like(mut self, col: impl ColumnTrait, val: &str) -> Self {
        self.select.and_where(col.like(val));
        self
    }

    /// WHERE col IN (vals...)
    pub fn where_in(mut self, col: impl ColumnTrait, vals: Vec<impl Into<Value>>) -> Self {
        let values: Vec<Value> = vals.into_iter().map(|v| v.into()).collect();
        self.select.and_where(col.is_in(values));
        self
    }

    /// WHERE col IS NULL
    pub fn where_null(mut self, col: impl ColumnTrait) -> Self {
        self.select.and_where(col.is_null());
        self
    }

    /// WHERE col IS NOT NULL
    pub fn where_not_null(mut self, col: impl ColumnTrait) -> Self {
        self.select.and_where(col.is_not_null());
        self
    }

    /// WHERE col BETWEEN low AND high
    pub fn where_between(
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
    /// Prefer the type-safe [`where_eq`](Self::where_eq) methods that accept the
    /// Column enum directly.
    pub fn where_str(mut self, col: &str, val: impl Into<Value>) -> Self {
        use sea_orm::sea_query::{BinOper, ColumnRef, DynIden, SimpleExpr};

        let val_expr: SimpleExpr = val.into().into();
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

// ── Scopes — reusable query fragments ─────────────────────────────────

/// A reusable query scope that can be applied to any [`QueryBuilder`].
///
/// # Example
///
/// ```rust,ignore
/// #[derive(Clone)]
/// struct Active;
///
/// impl Scope<User> for Active {
///     fn apply(self, qb: QueryBuilder) -> QueryBuilder {
///         qb.where_eq(UserColumn::Status, "active")
///     }
/// }
///
/// let users = User::query().scope(Active).get(&db).await?;
/// ```
pub trait Scope<T> {
    fn apply(self, qb: QueryBuilder) -> QueryBuilder;
}

impl QueryBuilder {
    /// Apply a scope to this query.  Scopes are reusable query fragments.
    pub fn scope<T>(self, scope: impl Scope<T>) -> Self {
        scope.apply(self)
    }
}

// ── ORDER BY, LIMIT, OFFSET ────────────────────────────────────────────

impl QueryBuilder {
    /// ORDER BY col (Direction)
    pub fn order_by(mut self, col: impl ColumnTrait, order: Order) -> Self {
        let (_, col_name) = col.as_column_ref();
        self.select
            .order_by(sea_query::ColumnRef::Column(col_name.into()), order);
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

// ── Aggregates ─────────────────────────────────────────────────────────

impl QueryBuilder {
    /// SELECT SUM(col) — Column enum or `&str`.
    pub async fn sum(self, col: impl ColumnTrait, db: &DatabaseConnection) -> Result<f64> {
        self.aggregate(col, "SUM", db).await
    }

    /// SELECT AVG(col) — Column enum or `&str`.
    pub async fn avg(self, col: impl ColumnTrait, db: &DatabaseConnection) -> Result<f64> {
        self.aggregate(col, "AVG", db).await
    }

    /// SELECT MIN(col) — Column enum or `&str`.
    pub async fn min(self, col: impl ColumnTrait, db: &DatabaseConnection) -> Result<f64> {
        self.aggregate(col, "MIN", db).await
    }

    /// SELECT MAX(col) — Column enum or `&str`.
    pub async fn max(self, col: impl ColumnTrait, db: &DatabaseConnection) -> Result<f64> {
        self.aggregate(col, "MAX", db).await
    }

    async fn aggregate(
        self,
        col: impl ColumnTrait,
        func: &str,
        db: &DatabaseConnection,
    ) -> Result<f64> {
        let (_, col_name) = col.as_column_ref();
        let col_str = col_name.to_string();
        let backend = db.get_database_backend();
        let inner = Self::build_sql(&self.select, backend);
        let sql = format!("SELECT {}(\"{}\") FROM ({}) AS sub", func, col_str, inner);
        let stmt = Statement::from_string(backend, sql);
        let rows = db
            .query_all_raw(stmt)
            .await
            .map_err(RavelEloquentError::Database)?;
        Ok(rows
            .first()
            .and_then(|r| {
                r.try_get_by_index::<f64>(0)
                    .or_else(|_| r.try_get_by_index::<i64>(0).map(|v| v as f64))
                    .ok()
            })
            .unwrap_or(0.0))
    }

    /// GROUP BY col — Column enum or `&str`.
    pub fn group_by(mut self, col: impl ColumnTrait) -> Self {
        let (_, col_name) = col.as_column_ref();
        let col_ref: sea_query::ColumnRef = sea_query::DynIden::from(col_name.to_string()).into();
        self.select.add_group_by([Expr::col(col_ref)]);
        self
    }
}

// ── whereHas / orWhereHas — filter by existence of related records ─────

impl QueryBuilder {
    /// Filter records that have at least one matching related record
    /// through the named relation.
    ///
    /// The closure receives a sub-query builder on the related table
    /// and can add arbitrary WHERE conditions.
    ///
    /// ```rust,ignore
    /// // Users who have at least one published post
    /// let users = User::query()
    ///     .where_has::<User>("posts", |qb| {
    ///         qb.where_eq(PostColumn::Published, true)
    ///     })
    ///     .get(&db).await?;
    /// ```
    pub fn where_has<T: ModelMeta>(
        self,
        rel_name: &str,
        f: impl FnOnce(QueryBuilder) -> QueryBuilder,
    ) -> Self {
        self.has_relation::<T>(rel_name, f, false)
    }

    /// OR variant of [`where_has`](Self::where_has).
    pub fn or_where_has<T: ModelMeta>(
        self,
        rel_name: &str,
        f: impl FnOnce(QueryBuilder) -> QueryBuilder,
    ) -> Self {
        self.has_relation::<T>(rel_name, f, true)
    }

    fn has_relation<T: ModelMeta>(
        mut self,
        rel_name: &str,
        f: impl FnOnce(QueryBuilder) -> QueryBuilder,
        or: bool,
    ) -> Self {
        let meta = T::get_relation(rel_name).unwrap_or_else(|| {
            panic!(
                "where_has: relation '{}' not found on model '{}'",
                rel_name,
                T::table_name()
            )
        });

        if meta.kind == RelationKind::BelongsToMany {
            panic!(
                "where_has: BelongsToMany relation '{}' is not yet supported. \
                 Use a manual EXISTS subquery instead.",
                rel_name
            );
        }

        // Build subquery: SELECT 1 FROM related
        //                 WHERE related.fk = parent.pk AND <user filter>
        let mut sub = Query::select();
        sub.expr(Expr::cust("1"));
        sub.from(sea_query::Alias::new(meta.table_name));

        // Correlation: related.fk = parent.pk  — identifiers are static
        let corr_sql = format!(
            "\"{}\".\"{}\" = \"{}\".\"{}\"",
            meta.table_name, meta.foreign_key, T::table_name(), meta.local_key,
        );
        sub.and_where(Expr::cust(corr_sql));

        // Apply user's filter via QueryBuilder
        let tmp = QueryBuilder { select: sub, columns: vec![], eager_loads: vec![] };
        let tmp = f(tmp);

        if or {
            self.select.cond_where(
                sea_orm::sea_query::Condition::any().add(Expr::exists(tmp.select)),
            );
        } else {
            self.select.and_where(Expr::exists(tmp.select));
        }
        self
    }
}

// ── Execution ──────────────────────────────────────────────────────────

impl QueryBuilder {
    /// Execute the query and return all matching rows.
    pub async fn get<T: ModelExt>(self, db: &DatabaseConnection) -> Result<Vec<T>> {
        let eager = self.eager_loads.clone();
        let mut items = self.run(db).await?;
        for rel in &eager {
            T::load_relation(rel, &mut items, db).await?;
        }
        Ok(items)
    }

    /// Execute the query and return the first matching row, if any.
    pub async fn first<T: ModelExt>(
        mut self,
        db: &DatabaseConnection,
    ) -> Result<Option<T>> {
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
            .map_err(RavelEloquentError::Database)?;
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
    pub async fn paginate<T: ModelExt>(
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

pub type Page<T> = ravel_db_seaorm::Page<T>;

// ── SQL helpers (kept for relations.rs and ModelExt) ───────────────────

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

pub fn try_extract(row: &sea_orm::QueryResult, index: usize) -> serde_json::Value {
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
