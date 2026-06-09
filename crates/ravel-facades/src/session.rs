//! Session facade — encrypted cookie session access.
//! Uses REQUEST task-local (shared with Session extractor via SessionState).
//! Outside HTTP scope: all methods return None / are no-ops.

use ravel_http::facades::REQUEST;

pub struct Session;

impl Session {
    pub fn get<T: serde::de::DeserializeOwned>(key: &str) -> Option<T> {
        REQUEST
            .try_with(|ctx| {
                let guard = ctx.session.data.lock().ok()?;
                guard
                    .values
                    .get(key)
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
            })
            .unwrap_or(None)
    }

    pub fn put<T: serde::Serialize>(key: &str, value: &T) {
        let _ = REQUEST.try_with(|ctx| {
            if let Ok(val) = serde_json::to_value(value)
                && let Ok(mut guard) = ctx.session.data.lock()
            {
                guard.values.insert(key.to_string(), val);
                ctx.session.mark_dirty();
            }
        });
    }

    pub fn has(key: &str) -> bool {
        REQUEST
            .try_with(|ctx| {
                ctx.session
                    .data
                    .lock()
                    .is_ok_and(|guard| guard.values.contains_key(key))
            })
            .unwrap_or(false)
    }

    pub fn forget(key: &str) {
        let _ = REQUEST.try_with(|ctx| {
            if let Ok(mut guard) = ctx.session.data.lock() {
                guard.values.remove(key);
                ctx.session.mark_dirty();
            }
        });
    }

    pub fn flash<T: serde::Serialize>(key: &str, value: &T) {
        let _ = REQUEST.try_with(|ctx| {
            if let Ok(val) = serde_json::to_value(value)
                && let Ok(mut guard) = ctx.session.data.lock()
            {
                guard.flash.insert(key.to_string(), val);
                ctx.session.mark_dirty();
            }
        });
    }

    pub fn flashed<T: serde::de::DeserializeOwned>(key: &str) -> Option<T> {
        REQUEST
            .try_with(|ctx| {
                let mut guard = ctx.session.data.lock().ok()?;
                guard
                    .flash
                    .remove(key)
                    .and_then(|v| serde_json::from_value(v).ok())
            })
            .unwrap_or(None)
    }
}
