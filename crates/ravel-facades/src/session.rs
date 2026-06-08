//! Session facade — encrypted cookie session access.
//! Uses REQUEST task-local. Outside HTTP scope: all methods return None / are no-ops.

use ravel_http::facades::REQUEST;

pub struct Session;

impl Session {
    pub fn get<T: serde::de::DeserializeOwned>(key: &str) -> Option<T> {
        REQUEST
            .try_with(|ctx| {
                let session = ctx.session.lock();
                session
                    .values
                    .get(key)
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
            })
            .unwrap_or(None)
    }

    pub fn put<T: serde::Serialize>(key: &str, value: &T) {
        let _ = REQUEST.try_with(|ctx| {
            if let Ok(val) = serde_json::to_value(value) {
                ctx.session.lock().values.insert(key.to_string(), val);
            }
        });
    }

    pub fn has(key: &str) -> bool {
        REQUEST
            .try_with(|ctx| ctx.session.lock().values.contains_key(key))
            .unwrap_or(false)
    }

    pub fn forget(key: &str) {
        let _ = REQUEST.try_with(|ctx| {
            ctx.session.lock().values.remove(key);
        });
    }

    pub fn flash<T: serde::Serialize>(key: &str, value: &T) {
        let _ = REQUEST.try_with(|ctx| {
            if let Ok(val) = serde_json::to_value(value) {
                ctx.session.lock().flash.insert(key.to_string(), val);
            }
        });
    }

    pub fn flashed<T: serde::de::DeserializeOwned>(key: &str) -> Option<T> {
        REQUEST
            .try_with(|ctx| {
                let mut session = ctx.session.lock();
                session
                    .flash
                    .remove(key)
                    .and_then(|v| serde_json::from_value(v).ok())
            })
            .unwrap_or(None)
    }
}
