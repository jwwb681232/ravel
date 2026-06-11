use ravel_http::controller::Controller;
use ravel_core::container::Container;
use ravel_http::form_request::Validated;
use ravel_http::error::RavelError;
use axum::response::IntoResponse;
use crate::app::http::requests::create_post_request::CreatePostRequest;

pub struct PostController;

impl Controller for PostController {
    fn boot(_container: &Container) -> Self { Self }
}

impl PostController {
    pub async fn store(
        Validated(req): Validated<CreatePostRequest>,
    ) -> Result<impl IntoResponse, RavelError> {
        Ok(format!("Post created: {} — {}", req.title, req.content))
    }
}
