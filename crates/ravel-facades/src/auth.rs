//! Auth facade — authentication helpers for the current request.
//! Uses the shared session state (via REQUEST task-local) so auth state
//! is consistent with the Session extractor. Outside HTTP scope: all
//! methods return false/None.

use ravel_http::facades::REQUEST;

pub struct Auth;

impl Auth {
    pub fn check() -> bool {
        REQUEST
            .try_with(|ctx| {
                ctx.session
                    .data
                    .lock()
                    .is_ok_and(|guard| guard.values.contains_key("_auth_id"))
            })
            .unwrap_or(false)
    }

    pub fn guest() -> bool {
        !Self::check()
    }

    pub fn id<T: serde::de::DeserializeOwned>() -> Option<T> {
        REQUEST
            .try_with(|ctx| {
                let guard = ctx.session.data.lock().ok()?;
                guard
                    .values
                    .get("_auth_id")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
            })
            .unwrap_or(None)
    }

    pub fn login<T: serde::Serialize>(id: &T) {
        let _ = REQUEST.try_with(|ctx| {
            if let Ok(val) = serde_json::to_value(id)
                && let Ok(mut guard) = ctx.session.data.lock()
            {
                guard.values.insert("_auth_id".into(), val);
                ctx.session.mark_dirty();
            }
        });
    }

    pub fn logout() {
        let _ = REQUEST.try_with(|ctx| {
            if let Ok(mut guard) = ctx.session.data.lock() {
                guard.values.remove("_auth_id");
                ctx.session.mark_dirty();
            }
        });
    }
}
