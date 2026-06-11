use ravel_facades::Route;
use crate::app::http::controllers::user_controller::UserController;
use crate::app::http::controllers::post_controller::PostController;

pub fn register() {
    // Home
    Route::get("/", || async { "Hello, {{name}}! 🚀" });

    // Auth (public)
    Route::post("/login", UserController::login);
    Route::post("/logout", UserController::logout);

    // Protected API group
    Route::group("/api", || {
        Route::get("/users", UserController::index);
        Route::get("/users/{id}", UserController::show);
        Route::post("/posts", PostController::store);
    });
}
