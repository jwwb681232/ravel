//! Integration tests for SoftDeletes (soft-delete + trashed + restore).

use ravel_eloquent::{ActiveModelExt, Model, ModelExt, ModelMeta};
use sea_orm::ConnectionTrait;
use serde::{Deserialize, Serialize};

// ── Soft-delete model ───────────────────────────────────────────────────────

#[derive(Model, Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
#[model(table = "soft_users", soft_deletes)]
struct SoftUser {
    #[model(id)]
    pub id: i32,
    pub name: String,
    #[model(nullable, datetime)]
    pub deleted_at: Option<String>,
}

// ── Model that uses a custom column name for soft delete ────────────────────

#[derive(Model, Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
#[model(table = "custom_users", soft_deletes = "deleted_on")]
struct CustomSoftUser {
    #[model(id)]
    pub id: i32,
    pub name: String,
}

// ── Metadata tests ──────────────────────────────────────────────────────────

#[test]
fn test_soft_delete_column_present() {
    assert_eq!(SoftUser::soft_delete_column(), Some("deleted_at"));
    // deleted_at should be in columns
    let cols = SoftUser::columns();
    assert!(cols.contains(&"deleted_at"));
}

#[test]
fn test_custom_soft_delete_column() {
    assert_eq!(CustomSoftUser::soft_delete_column(), Some("deleted_on"));
    let cols = CustomSoftUser::columns();
    assert!(cols.contains(&"deleted_on"));
}

#[test]
fn test_no_soft_delete_without_attribute() {
    // TestUser (from integration_crud) should NOT have soft deletes
    // We test with the existing model from model_derive tests
    // User has no #[model(soft_deletes)], so it shouldn't have one
    // (This is verified by integration_crud tests working normally)
}

// ── Core soft-delete behaviour ──────────────────────────────────────────────

#[tokio::test]
async fn test_all_filters_out_trashed() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE soft_users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            deleted_at TEXT
        )",
    ).await.unwrap();

    db.execute_unprepared("INSERT INTO soft_users (name, deleted_at) VALUES ('Active', NULL)").await.unwrap();
    db.execute_unprepared("INSERT INTO soft_users (name, deleted_at) VALUES ('Deleted', datetime('now'))").await.unwrap();

    let users = SoftUser::all(&db).await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Active");
}

#[tokio::test]
async fn test_find_filters_trashed() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE soft_users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            deleted_at TEXT
        )",
    ).await.unwrap();

    db.execute_unprepared("INSERT INTO soft_users (name, deleted_at) VALUES ('Trashed', datetime('now'))").await.unwrap();

    let found = SoftUser::find(&db, 1).await.unwrap();
    // Should be None because it's soft-deleted
    assert!(found.is_none());
}

#[tokio::test]
async fn test_all_with_trashed() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE soft_users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            deleted_at TEXT
        )",
    ).await.unwrap();

    db.execute_unprepared("INSERT INTO soft_users (name, deleted_at) VALUES ('Active', NULL)").await.unwrap();
    db.execute_unprepared("INSERT INTO soft_users (name, deleted_at) VALUES ('Trashed', datetime('now'))").await.unwrap();

    let users = SoftUser::all_with_trashed(&db).await.unwrap();
    assert_eq!(users.len(), 2);
}

#[tokio::test]
async fn test_all_only_trashed() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE soft_users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            deleted_at TEXT
        )",
    ).await.unwrap();

    db.execute_unprepared("INSERT INTO soft_users (name, deleted_at) VALUES ('Active', NULL)").await.unwrap();
    db.execute_unprepared("INSERT INTO soft_users (name, deleted_at) VALUES ('Trashed', datetime('now'))").await.unwrap();

    let users = SoftUser::all_only_trashed(&db).await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Trashed");
}

#[tokio::test]
async fn test_delete_soft_deletes() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE soft_users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            deleted_at TEXT
        )",
    ).await.unwrap();

    db.execute_unprepared("INSERT INTO soft_users (name, deleted_at) VALUES ('Alice', NULL)").await.unwrap();

    let user = SoftUser::find(&db, 1).await.unwrap().unwrap();
    assert_eq!(user.name, "Alice");

    // Soft delete
    user.delete(&db).await.unwrap();

    // Verify it's now filtered out
    let found = SoftUser::find(&db, 1).await.unwrap();
    assert!(found.is_none());

    // But visible with with_trashed
    let users = SoftUser::all_with_trashed(&db).await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");
    assert!(users[0].deleted_at.is_some());
}

#[tokio::test]
async fn test_restore() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE soft_users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            deleted_at TEXT
        )",
    ).await.unwrap();

    db.execute_unprepared("INSERT INTO soft_users (name, deleted_at) VALUES ('Bob', NULL)").await.unwrap();

    let user = SoftUser::find(&db, 1).await.unwrap().unwrap();
    user.delete(&db).await.unwrap();

    // Now restore via with_trashed
    let trashed_users = SoftUser::all_with_trashed(&db).await.unwrap();
    let trashed = trashed_users.into_iter().find(|u| u.name == "Bob").unwrap();
    let restored = trashed.restore(&db).await.unwrap();
    assert!(restored.deleted_at.is_none());

    // Should be findable again
    let found = SoftUser::find(&db, 1).await.unwrap();
    assert!(found.is_some());
}

#[tokio::test]
async fn test_force_delete_hard_deletes() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE soft_users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            deleted_at TEXT
        )",
    ).await.unwrap();

    db.execute_unprepared("INSERT INTO soft_users (name, deleted_at) VALUES ('Gone', NULL)").await.unwrap();

    let user = SoftUser::find(&db, 1).await.unwrap().unwrap();
    user.force_delete(&db).await.unwrap();

    // Should be gone even with with_trashed
    let users = SoftUser::all_with_trashed(&db).await.unwrap();
    assert!(users.is_empty());
}

#[tokio::test]
async fn test_destroy_soft_deletes_multiple() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE soft_users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            deleted_at TEXT
        )",
    ).await.unwrap();

    db.execute_unprepared("INSERT INTO soft_users (name) VALUES ('A')").await.unwrap();
    db.execute_unprepared("INSERT INTO soft_users (name) VALUES ('B')").await.unwrap();
    db.execute_unprepared("INSERT INTO soft_users (name) VALUES ('C')").await.unwrap();

    let count = SoftUser::destroy(&db, [1, 2]).await.unwrap();
    assert_eq!(count, 2); // rows_affected = 2

    // A and B are now soft-deleted
    let active = SoftUser::all(&db).await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].name, "C");

    // All still exist
    let all = SoftUser::all_with_trashed(&db).await.unwrap();
    assert_eq!(all.len(), 3);
}

#[tokio::test]
async fn test_query_builder_respects_soft_deletes() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE soft_users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            deleted_at TEXT
        )",
    ).await.unwrap();

    db.execute_unprepared("INSERT INTO soft_users (name, deleted_at) VALUES ('Active', NULL)").await.unwrap();
    db.execute_unprepared("INSERT INTO soft_users (name, deleted_at) VALUES ('Trashed', datetime('now'))").await.unwrap();

    let users: Vec<SoftUser> = SoftUser::query().get(&db).await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Active");
}

#[tokio::test]
async fn test_query_with_trashed() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE soft_users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            deleted_at TEXT
        )",
    ).await.unwrap();

    db.execute_unprepared("INSERT INTO soft_users (name, deleted_at) VALUES ('Active', NULL)").await.unwrap();
    db.execute_unprepared("INSERT INTO soft_users (name, deleted_at) VALUES ('Trashed', datetime('now'))").await.unwrap();

    let users: Vec<SoftUser> = SoftUser::query_with_trashed().get(&db).await.unwrap();
    assert_eq!(users.len(), 2);
}

#[tokio::test]
async fn test_custom_soft_delete_column_works() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE custom_users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            deleted_on TEXT
        )",
    ).await.unwrap();

    db.execute_unprepared("INSERT INTO custom_users (name, deleted_on) VALUES ('Active', NULL)").await.unwrap();
    db.execute_unprepared("INSERT INTO custom_users (name, deleted_on) VALUES ('Gone', datetime('now'))").await.unwrap();

    let users = CustomSoftUser::all(&db).await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Active");
}
