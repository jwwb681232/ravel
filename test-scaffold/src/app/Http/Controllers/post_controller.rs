use ravel_http::controller::Controller;
use ravel_core::container::Container;
use axum::response::IntoResponse;

pub struct PostController;

impl Controller for PostController {
    fn boot(_container: &Container) -> Self { Self }
}

impl PostController {
    pub async fn store(
        axum::extract::Json(body): axum::extract::Json<serde_json::Value>,
    ) -> impl IntoResponse {
        format!("Post created: {body}")
    }
}
