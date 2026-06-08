use std::time::Duration;

pub struct Cache;

impl Cache {
    pub fn put(_key: &str, _value: impl std::any::Any + Send + Sync, _ttl: Option<Duration>) {
        unimplemented!()
    }
    pub fn get<T: 'static + Clone + Send + Sync>(_key: &str) -> Option<T> {
        unimplemented!()
    }
    pub fn has(_key: &str) -> bool {
        unimplemented!()
    }
    pub fn forget(_key: &str) {
        unimplemented!()
    }
    pub fn flush() {
        unimplemented!()
    }
}
