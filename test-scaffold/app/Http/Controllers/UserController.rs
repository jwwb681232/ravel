use ravel_http::controller::Controller;
use ravel_core::container::Container;
use ravel_facades::{Auth, Session, Config, Log, response, redirect};
use ravel_http::error::RavelError;
use axum::response::IntoResponse;
use ravel_http::session::Session as SessionExt;

pub struct UserController;

impl Controller for UserController {
    fn boot(_container: &Container) -> Self { Self }
}

impl UserController {
    /// GET /users — show usage of Config and Log facades
    pub async fn index() -> impl IntoResponse {
        let app_name: String = Config::get_or("app.name", "test-scaffold");
        Log::info!("User list requested in {app_name}");
        response()
            .json(serde_json::json!({"users": ["alice", "bob"]}))
            .unwrap()
    }

    /// GET /users/{id} — requires login; shows Auth facade + RavelError
    pub async fn show(session: SessionExt, id: u32) -> Result<impl IntoResponse, RavelError> {
        if Auth::guest(&session) {
            return Ok(redirect("/login"));
        }
        let user_id: Option<i32> = Auth::id(&session);
        Log::info!("User {id} viewed by {user_id:?}");
        Ok(response()
            .json(serde_json::json!({"id": id, "name": "Alice"}))
            .unwrap())
    }

    /// POST /login — demonstrate Auth::login + Session::flash
    pub async fn login(mut session: SessionExt) -> impl IntoResponse {
        Auth::login(&mut session, 1);
        Session::flash(&mut session, "status", "Welcome back!");
        redirect("/")
    }

    /// POST /logout
    pub async fn logout(mut session: SessionExt) -> impl IntoResponse {
        Auth::logout(&mut session);
        redirect("/")
    }
}
