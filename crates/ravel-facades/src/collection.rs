pub struct Collection<T> {
    items: Vec<T>,
}

impl<T> Collection<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self { items }
    }
    pub fn to_vec(self) -> Vec<T> {
        self.items
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
