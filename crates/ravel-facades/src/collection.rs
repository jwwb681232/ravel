//! Collection pipeline — Laravel-style iterable wrapper.
//! Usage: collect!(vec![...]).map(...).filter(...).to_vec()
#![allow(clippy::needless_borrow)]

pub struct Collection<T> {
    items: Vec<T>,
}

impl<T> Collection<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self { items }
    }

    pub fn map<U>(self, f: impl FnMut(T) -> U) -> Collection<U> {
        Collection {
            items: self.items.into_iter().map(f).collect(),
        }
    }

    pub fn filter(self, f: impl FnMut(&T) -> bool) -> Self {
        Self {
            items: self.items.into_iter().filter(f).collect(),
        }
    }

    pub fn reject(self, mut f: impl FnMut(&T) -> bool) -> Self {
        Self {
            items: self.items.into_iter().filter(|x| !f(x)).collect(),
        }
    }

    pub fn first(&self) -> Option<&T> {
        self.items.first()
    }

    pub fn last(&self) -> Option<&T> {
        self.items.last()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn is_not_empty(&self) -> bool {
        !self.items.is_empty()
    }

    pub fn count(&self) -> usize {
        self.items.len()
    }

    pub fn to_vec(self) -> Vec<T> {
        self.items
    }

    pub fn take(self, n: usize) -> Self {
        Self {
            items: self.items.into_iter().take(n).collect(),
        }
    }

    pub fn skip(self, n: usize) -> Self {
        Self {
            items: self.items.into_iter().skip(n).collect(),
        }
    }

    pub fn contains(&self, f: impl FnMut(&T) -> bool) -> bool {
        self.items.iter().any(f)
    }

    pub fn each(self, mut f: impl FnMut(&T)) -> Self {
        for item in &self.items {
            f(item);
        }
        self
    }
}

impl<T: Ord> Collection<T> {
    pub fn sort(self) -> Self {
        let mut items = self.items;
        items.sort();
        Self { items }
    }
}

impl<T: std::fmt::Display> Collection<T> {
    pub fn implode(&self, glue: &str) -> String {
        self.items
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(glue)
    }
}

impl<T> From<Vec<T>> for Collection<T> {
    fn from(items: Vec<T>) -> Self {
        Self { items }
    }
}

impl<T> From<Collection<T>> for Vec<T> {
    fn from(c: Collection<T>) -> Self {
        c.items
    }
}

#[macro_export]
macro_rules! collect {
    ($vec:expr) => {
        $crate::collection::Collection::new($vec)
    };
}
