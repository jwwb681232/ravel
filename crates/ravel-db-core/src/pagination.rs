//! Generic pagination support.
//!
//! Provides [`Page`] — a simple wrapper for paginated query results.
//! Individual database backends provide their own paginator implementations.

/// A page of query results with metadata.
#[derive(Debug, Clone)]
pub struct Page<T> {
    /// The items in this page.
    pub items: Vec<T>,
    /// Total number of items across all pages.
    pub total: u64,
    /// Current page number (1-based).
    pub page: u64,
    /// Number of items per page.
    pub per_page: u64,
}

impl<T> Page<T> {
    /// Create a new page.
    pub fn new(items: Vec<T>, total: u64, page: u64, per_page: u64) -> Self {
        Self {
            items,
            total,
            page,
            per_page,
        }
    }

    /// Total number of pages.
    pub fn last_page(&self) -> u64 {
        if self.per_page == 0 {
            return 1;
        }
        (self.total + self.per_page - 1) / self.per_page
    }

    /// Is there a next page?
    pub fn has_more(&self) -> bool {
        self.page < self.last_page()
    }

    /// Number of items in this page.
    pub fn count(&self) -> usize {
        self.items.len()
    }

    /// Is this page empty?
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
