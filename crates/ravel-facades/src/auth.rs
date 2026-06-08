pub struct Auth;

impl Auth {
    pub fn check() -> bool {
        unimplemented!()
    }
    pub fn guest() -> bool {
        unimplemented!()
    }
    pub fn id<T: serde::de::DeserializeOwned>() -> Option<T> {
        unimplemented!()
    }
    pub fn login<T: serde::Serialize>(_id: &T) {
        unimplemented!()
    }
    pub fn logout() {
        unimplemented!()
    }
}
