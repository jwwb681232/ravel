use axum::Router;
use ravel_http::route::Route;

#[tokio::main]
async fn main() {
    let app: Router = Route::new()
        .get("/", || async { "Hello, Ravel! 🚀" })
        .build();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("Ravel running at http://127.0.0.1:3000");

    axum::serve(listener, app).await.unwrap();
}
