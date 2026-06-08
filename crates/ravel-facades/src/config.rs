use serde::de::DeserializeOwned;

pub struct Config;

impl Config {
    pub fn get<T: DeserializeOwned>(_key: &str) -> Option<T> {
        unimplemented!()
    }
    pub fn get_or<T: DeserializeOwned>(_key: &str, _default: T) -> T {
        unimplemented!()
    }
    pub fn has(_key: &str) -> bool {
        unimplemented!()
    }
}
