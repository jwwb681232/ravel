// routes/web.rs — Web routes
//
// Register your web routes here using the Ravel Route DSL.

use ravel_http::route::Route;

pub fn routes() -> Route {
    Route::new()
        .get("/", || async { "Hello, Ravel!" })
}
