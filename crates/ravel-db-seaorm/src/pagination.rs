//! Pagination wrapper — Ravel-style paginator on top of SeaORM.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_db_seaorm::pagination::Paginator;
//! use sea_orm::*;
//!
//! let page = Paginator::new(Users::find().order_by_asc(users::Column::Id))
//!     .per_page(15)
//!     .page(&db, 1)
//!     .await?;
//!
//! println!("Page {} of page {}", page.page, page.last_page());
//! for user in page.items {
//!     println!("{}: {}", user.id, user.name);
//! }
//! ```

use anyhow::Result;
use crate::page::Page;
use sea_orm::{DatabaseConnection, PaginatorTrait, Select};

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
        let _num_pages = paginator.num_pages().await?;

        let page_idx = if page == 0 { 0 } else { page.saturating_sub(1) };
        let items = paginator.fetch_page(page_idx).await?;

        Ok(Page::new(
            items,
            num_items as u64,
            page.max(1),
            self.per_page,
        ))
    }

    /// Simple paginate: `Paginator::simple(select, &db, 1, 15).await`
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
