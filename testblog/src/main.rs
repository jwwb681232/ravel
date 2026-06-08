#[path = "../bootstrap/mod.rs"]
mod bootstrap;
#[path = "../routes/mod.rs"]
mod routes;

use axum::{routing::get, Router};

#[tokio::main]
async fn main() {
    bootstrap::app::create_app();
    let _ = routes::web::routes();

    let app = Router::new().route("/", get(|| async { "Hello, Ravel!" }));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("🚀 Server running at http://127.0.0.1:3000");

    axum::serve(listener, app).await.unwrap();
}
