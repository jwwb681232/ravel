use ravel_facades::Route;
use ravel_http::middleware::log_requests;
use crate::app::Http::Controllers::{UserController, PostController};

pub fn register() {
    // Global middleware: log all requests
    Route::middleware(log_requests);

    // Home
    Route::get("/", || async { "Hello, test-scaffold! 🚀" });

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
