//! Pagination wrapper — Ravel-style paginator on top of SeaORM.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_db::pagination::Paginator;
//! use ravel_db::sea_orm::*;
//!
//! let page = Paginator::new(Users::find().order_by_asc(users::Column::Id))
//!     .per_page(15)
//!     .page(&db, 1)
//!     .await?;
//!
//! println!("Page {} of {}", page.current, page.last);
//! for user in page.items {
//!     println!("{}: {}", user.id, user.name);
//! }
//! ```

use anyhow::Result;
use sea_orm::{DatabaseConnection, PaginatorTrait, Select};

/// A typed page of results.
#[derive(Debug)]
pub struct Page<T> {
    /// Items for the current page.
    pub items: Vec<T>,
    /// 1-based current page number.
    pub current: u64,
    /// Total number of pages.
    pub last: u64,
    /// Total number of items.
    pub total: u64,
    /// Items per page.
    pub per_page: u64,
}

/// Build a paginated query.
///
/// Wraps SeaORM's [`PaginatorTrait`] with a Ravel-style fluent API.
pub struct Paginator<E>
where
    E: sea_orm::EntityTrait,
{
    select: Select<E>,
    per_page: u64,
}

impl<E: sea_orm::EntityTrait> Paginator<E> {
    /// Create a paginator from a SeaORM `Select`.
    pub fn new(select: Select<E>) -> Self {
        Self {
            select,
            per_page: 15, // Laravel default
        }
    }

    /// Items per page (default: 15).
    pub fn per_page(mut self, n: u64) -> Self {
        self.per_page = n;
        self
    }

    /// Fetch a specific page (1-based).
    pub async fn page(self, db: &DatabaseConnection, page: u64) -> Result<Page<E::Model>>
    where
        <E as sea_orm::EntityTrait>::Model: Send + Sync,
    {
        let paginator = self.select.paginate(db, self.per_page);

        let num_items = paginator.num_items().await?;
        let num_pages = paginator.num_pages().await?;

        let page_idx = if page == 0 { 0 } else { page.saturating_sub(1) };
        let items = paginator.fetch_page(page_idx).await?;

        Ok(Page {
            items,
            current: page.max(1),
            last: num_pages.max(1),
            total: num_items as u64,
            per_page: self.per_page,
        })
    }

    /// Simple paginate: `Paginator::from(select).simple(&db, 1, 15).await`
    pub async fn simple(
        select: Select<E>,
        db: &DatabaseConnection,
        page: u64,
        per_page: u64,
    ) -> Result<Page<E::Model>>
    where
        <E as sea_orm::EntityTrait>::Model: Send + Sync,
    {
        Self::new(select).per_page(per_page).page(db, page).await
    }
}

impl<T> Page<T> {
    /// Check if there are more pages after this one.
    pub fn has_more(&self) -> bool {
        self.current < self.last
    }

    /// Check if there are previous pages.
    pub fn has_previous(&self) -> bool {
        self.current > 1
    }

    /// Returns true if this page is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Number of items on this page.
    pub fn count(&self) -> usize {
        self.items.len()
    }
}
