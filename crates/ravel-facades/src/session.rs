pub struct Session;

impl Session {
    pub fn get<T: serde::de::DeserializeOwned>(_key: &str) -> Option<T> {
        unimplemented!()
    }
    pub fn put<T: serde::Serialize>(_key: &str, _value: &T) {
        unimplemented!()
    }
    pub fn has(_key: &str) -> bool {
        unimplemented!()
    }
    pub fn forget(_key: &str) {
        unimplemented!()
    }
    pub fn flash<T: serde::Serialize>(_key: &str, _value: &T) {
        unimplemented!()
    }
    pub fn flashed<T: serde::de::DeserializeOwned>(_key: &str) -> Option<T> {
        unimplemented!()
    }
}
