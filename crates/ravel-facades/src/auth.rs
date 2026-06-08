//! Auth facade — authentication helpers for the current request.
//! Uses REQUEST task-local. Outside HTTP scope: all methods return false/None.

use ravel_http::facades::REQUEST;

pub struct Auth;

impl Auth {
    pub fn check() -> bool {
        REQUEST
            .try_with(|ctx| ctx.auth_id.lock().is_some())
            .unwrap_or(false)
    }

    pub fn guest() -> bool {
        !Self::check()
    }

    pub fn id<T: serde::de::DeserializeOwned>() -> Option<T> {
        REQUEST
            .try_with(|ctx| {
                let auth_id = ctx.auth_id.lock();
                auth_id.as_ref().and_then(|id| serde_json::from_str(id).ok())
            })
            .unwrap_or(None)
    }

    pub fn login<T: serde::Serialize>(id: &T) {
        let _ = REQUEST.try_with(|ctx| {
            if let Ok(json) = serde_json::to_string(id) {
                *ctx.auth_id.lock() = Some(json);
            }
        });
    }

    pub fn logout() {
        let _ = REQUEST.try_with(|ctx| {
            *ctx.auth_id.lock() = None;
        });
    }
}
