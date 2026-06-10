//! Integration tests for CRUD operations using SQLite in-memory database.
//!
//! These tests exercise the full Eloquent CRUD API against a real database,
//! verifying that model instances can be created, read, updated, and deleted.

use ravel_eloquent::{ActiveModelExt, Model};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection};

async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();

    // Create the test_users table manually
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS test_users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            email TEXT NOT NULL
        )",
    )
    .await
    .unwrap();
    db
}

#[derive(Model, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[model(table = "test_users")]
struct TestUser {
    #[model(id)]
    pub id: i32,
    pub name: String,
    pub email: String,
}

#[tokio::test]
async fn test_create_and_find() {
    let db = setup_db().await;

    let user = TestUser::create(
        serde_json::json!({"name": "Alice", "email": "alice@test.com"}),
        &db,
    )
    .await
    .unwrap();

    assert!(user.id > 0);
    assert_eq!(user.name, "Alice");

    let found = TestUser::find(&db, user.id).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().email, "alice@test.com");
}

#[tokio::test]
async fn test_save_update_delete() {
    let db = setup_db().await;

    // Insert with id = 0 triggers INSERT path
    let user = TestUser {
        id: 0,
        name: "Old".into(),
        email: "old@t.com".into(),
    };
    let user = user.save(&db).await.unwrap();
    assert!(user.id > 0);

    // Update by changing name and calling save with the same id
    let user = TestUser {
        id: user.id,
        name: "New".into(),
        email: user.email.clone(),
    };
    let user = user.save(&db).await.unwrap();
    assert_eq!(user.name, "New");

    // Delete
    let deleted_id = user.id;
    user.delete(&db).await.unwrap();
    let found = TestUser::find(&db, deleted_id).await.unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn test_query_filter_count() {
    let db = setup_db().await;

    for i in 0..5 {
        TestUser::create(
            serde_json::json!({"name": format!("U{}", i), "email": format!("u{}@t.com", i)}),
            &db,
        )
        .await
        .unwrap();
    }

    let count = TestUser::query().count(&db).await.unwrap();
    assert_eq!(count, 5);
}

#[tokio::test]
async fn test_delete_by_id() {
    let db = setup_db().await;
    let user = TestUser::create(serde_json::json!({"name": "D", "email": "d@t.com"}), &db)
        .await
        .unwrap();

    TestUser::destroy(&db, [user.id]).await.unwrap();
    let found = TestUser::find(&db, user.id).await.unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn test_aggregates() {
    let db = setup_db().await;

    for i in 0..5 {
        TestUser::create(
            serde_json::json!({"name": format!("U{}", i), "email": format!("u{}@t.com", i)}), &db,
        ).await.unwrap();
    }

    let max_id: f64 = TestUser::query().max(TestUserColumn::Id, &db).await.unwrap();
    assert!(max_id >= 5.0);

    let min_id: f64 = TestUser::query().min(TestUserColumn::Id, &db).await.unwrap();
    assert!(min_id >= 1.0);

    let sum: f64 = TestUser::query().sum(TestUserColumn::Id, &db).await.unwrap();
    assert!(sum >= 15.0); // 1+2+3+4+5

    let avg: f64 = TestUser::query().avg(TestUserColumn::Id, &db).await.unwrap();
    assert!(avg >= 1.0);
}

#[tokio::test]
async fn test_group_by() {
    let db = setup_db().await;

    TestUser::create(serde_json::json!({"name": "A", "email": "a@t.com"}), &db).await.unwrap();
    TestUser::create(serde_json::json!({"name": "B", "email": "b@t.com"}), &db).await.unwrap();

    // group_by just tests it compiles and runs without error
    let users: Vec<TestUser> = TestUser::query()
        .group_by(TestUserColumn::Name)
        .get(&db)
        .await
        .unwrap();
    assert!(!users.is_empty());
}
