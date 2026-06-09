//! Lazy-loading relation query builder.
//!
//! Wraps SeaORM's `Select<R>` to provide a fluent API for loading related
//! models, mirroring the same pattern as [`QueryBuilder`](crate::query::QueryBuilder).

use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, Order, QueryFilter, QueryOrder, QuerySelect,
    Value,
};

use crate::error::{RavelEloquentError, Result};
use crate::query::Page;

/// Lazy-loading relation query builder.
///
/// Wraps a SeaORM `Select<R>` so you can filter, order, and paginate a related
/// entity before executing — the same fluent pattern as [`QueryBuilder`].
///
/// # Type parameters
///
/// * `R` — the SeaORM entity trait for the *related* table.
pub struct RelationQuery<R: EntityTrait> {
    select: sea_orm::Select<R>,
}

impl<R: EntityTrait> RelationQuery<R> {
    /// Create a new relation query using `R::find()`.
    pub fn new() -> Self {
        Self { select: R::find() }
    }

    /// Wrap an existing SeaORM `Select<R>`.
    pub fn from_select(select: sea_orm::Select<R>) -> Self {
        Self { select }
    }

    // ── WHERE conditions (type-safe column enum) ─────────────────────────

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

    // ── ORDER BY ─────────────────────────────────────────────────────────

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

    // ── Pagination ───────────────────────────────────────────────────────

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

    // ── Execution ────────────────────────────────────────────────────────

    /// Execute and return all matching related rows.
    pub async fn get(self, db: &DatabaseConnection) -> Result<Vec<R::Model>> {
        self.select
            .all(db)
            .await
            .map_err(RavelEloquentError::Database)
    }

    /// Execute and return the first matching related row, if any.
    pub async fn first(self, db: &DatabaseConnection) -> Result<Option<R::Model>> {
        self.select
            .one(db)
            .await
            .map_err(RavelEloquentError::Database)
    }

    /// Return the number of matching related rows.
    pub async fn count(self, db: &DatabaseConnection) -> Result<u64> {
        let items = self
            .select
            .all(db)
            .await
            .map_err(RavelEloquentError::Database)?;
        Ok(items.len() as u64)
    }

    /// Return whether any matching related rows exist.
    pub async fn exists(self, db: &DatabaseConnection) -> Result<bool> {
        self.count(db).await.map(|c| c > 0)
    }

    /// Execute and paginate the related rows.
    pub async fn paginate(
        self,
        db: &DatabaseConnection,
        page: u64,
        per_page: u64,
    ) -> Result<Page<R::Model>> {
        let all_items = self
            .select
            .all(db)
            .await
            .map_err(RavelEloquentError::Database)?;
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

impl<R: EntityTrait> Default for RelationQuery<R> {
    fn default() -> Self {
        Self::new()
    }
}
