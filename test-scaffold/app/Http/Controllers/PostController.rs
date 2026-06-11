use ravel_http::controller::Controller;
use ravel_core::container::Container;
use ravel_facades::{Queue, response, abort};
use ravel_http::error::RavelError;
use ravel_http::form_request::Validated;
use axum::response::IntoResponse;
use sea_orm::DatabaseConnection;

use crate::app::Http::Requests::CreatePostRequest;
use crate::app::Models::Post;
use crate::app::Jobs::SendWelcomeEmail;

pub struct PostController;

impl Controller for PostController {
    fn boot(_container: &Container) -> Self { Self }
}

impl PostController {
    /// POST /posts — FormRequest validation, Model::create, Queue::dispatch
    pub async fn store(
        Validated(req): Validated<CreatePostRequest>,
        db: DatabaseConnection,
    ) -> Result<impl IntoResponse, RavelError> {
        let post = Post::create(
            serde_json::to_value(&req).map_err(|e| abort(500, e.to_string()))?,
            &db,
        )
        .await
        .map_err(|e| abort(500, e.to_string()))?;

        Queue::dispatch(SendWelcomeEmail { user_id: req.user_id })?;

        Ok(response()
            .status(201)
            .json(post.to_public_json())
            .unwrap())
    }
}
