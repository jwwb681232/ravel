//! Request facade — access the current HTTP request via task-local.

use ravel_http::facades::REQUEST;

/// Get a proxy for the current HTTP request.
pub fn request() -> RequestProxy {
    RequestProxy
}

pub struct RequestProxy;

impl RequestProxy {
    pub fn query<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        REQUEST
            .try_with(|ctx| ctx.request.query(key))
            .unwrap_or(None)
    }

    pub fn header(&self, name: &str) -> Option<String> {
        REQUEST
            .try_with(|ctx| ctx.request.header(name).map(|s| s.to_string()))
            .unwrap_or(None)
    }

    pub fn path(&self) -> Option<String> {
        REQUEST
            .try_with(|ctx| Some(ctx.request.path().to_string()))
            .unwrap_or(None)
    }

    pub fn method(&self) -> Option<String> {
        REQUEST
            .try_with(|ctx| Some(ctx.request.method().to_string()))
            .unwrap_or(None)
    }

    pub fn wants_json(&self) -> bool {
        REQUEST
            .try_with(|ctx| ctx.request.wants_json())
            .unwrap_or(false)
    }
}
