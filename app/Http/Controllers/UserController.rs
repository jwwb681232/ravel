use ravel_http::controller::Controller;
use ravel_core::container::Container;

pub struct UserController;

impl Controller for UserController {
    fn boot(_container: &Container) -> Self {
        Self
    }
}

// ── Handlers ──────────────────────────────────────────────────────
//
// impl UserController {
//     pub async fn index() -> impl axum::response::IntoResponse {
//         "UserController index"
//     }
// }
