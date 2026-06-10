//! Generic pagination support.

/// A page of query results with metadata.
#[derive(Debug, Clone)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

impl<T> Page<T> {
    pub fn new(items: Vec<T>, total: u64, page: u64, per_page: u64) -> Self {
        Self { items, total, page, per_page }
    }

    pub fn last_page(&self) -> u64 {
        if self.per_page == 0 { 1 } else { self.total.div_ceil(self.per_page) }
    }

    pub fn has_more(&self) -> bool { self.page < self.last_page() }
    pub fn count(&self) -> usize { self.items.len() }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}
