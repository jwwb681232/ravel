pub fn request() -> RequestProxy {
    RequestProxy
}
pub struct RequestProxy;

impl RequestProxy {
    pub fn query<T: serde::de::DeserializeOwned>(&self, _key: &str) -> Option<T> {
        unimplemented!()
    }
}
