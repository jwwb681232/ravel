use ravel_http::form_request::FormRequest;
use ravel_http::validation::{FieldRule, Rule};
use serde::Deserialize;

/// Validation rules for creating a post.
#[derive(Debug, Deserialize)]
pub struct CreatePostRequest {
    pub title: String,
    pub content: String,
    pub user_id: i32,
}

impl FormRequest for CreatePostRequest {
    fn rules() -> Vec<FieldRule> {
        vec![
            FieldRule::new("title", vec![
                Rule::Required,
                Rule::Min(3),
                Rule::Max(200),
            ]),
            FieldRule::new("content", vec![Rule::Required]),
            FieldRule::new("user_id", vec![
                Rule::Required,
                Rule::Exists { table: "users", column: "id" },
            ]),
        ]
    }
}
