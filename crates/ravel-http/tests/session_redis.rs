//! Integration tests for Redis-backed session.
//! Run with: cargo test -p ravel-http --features redis --test session_redis -- --test-threads=1

use ravel_core::crypt::Crypt;
#[cfg(feature = "redis")]
use ravel_http::session::{SessionConfig, SessionData};

#[cfg(feature = "redis")]
fn test_crypt() -> Crypt {
    let mut key = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut key);
    Crypt::new(&key)
}

#[cfg(feature = "redis")]
async fn setup() -> SessionConfig {
    let client = redis::Client::open("redis://127.0.0.1:6379").unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();
    let keys: Vec<String> = redis::AsyncCommands::keys(&mut conn, "ravel:session:*").await.unwrap();
    if !keys.is_empty() {
        let _: () = redis::AsyncCommands::del(&mut conn, keys).await.unwrap();
    }

    SessionConfig::redis("redis://127.0.0.1:6379", test_crypt())
        .await
        .unwrap()
        .cookie_name("test_session")
}

#[cfg(feature = "redis")]
#[tokio::test]
async fn test_redis_session_read_write() {
    let config = setup().await;

    let sid = SessionConfig::generate_id();
    let mut data = SessionData::default();
    data.values.insert("user_id".into(), serde_json::json!(42));
    data.values.insert("name".into(), serde_json::json!("Alice"));

    config.write_redis(&sid, &data).await.unwrap();

    let restored = config.read_redis(&sid).await.unwrap();
    assert_eq!(restored.values.get("user_id").unwrap(), &serde_json::json!(42));
    assert_eq!(restored.values.get("name").unwrap(), &serde_json::json!("Alice"));
}

#[cfg(feature = "redis")]
#[tokio::test]
async fn test_redis_session_missing_returns_none() {
    let config = setup().await;
    let result = config.read_redis("nonexistent-id").await;
    assert!(result.is_none());
}

#[cfg(feature = "redis")]
#[tokio::test]
async fn test_redis_session_cookie_format() {
    let config = setup().await;
    let cookie = config.write_redis_cookie("abc123");
    assert!(cookie.contains("test_session=abc123"));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("Max-Age=7200"));
}
