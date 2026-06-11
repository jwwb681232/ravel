use ravel_http::controller::Controller;
use ravel_core::container::Container;
use ravel_facades::Config;
use axum::response::IntoResponse;

pub struct UserController;

impl Controller for UserController {
    fn boot(_container: &Container) -> Self { Self }
}

impl UserController {
    pub async fn index() -> impl IntoResponse {
        let name: String = Config::get_or("app.name", "MyApp".to_string());
        format!("Welcome to {name} -- User list")
    }

    pub async fn show(axum::extract::Path(id): axum::extract::Path<u32>) -> impl IntoResponse {
        format!("User {id}")
    }

    pub async fn login() -> impl IntoResponse {
        "Logged in (Auth placeholder)"
    }

    pub async fn logout() -> impl IntoResponse {
        "Logged out"
    }
}
