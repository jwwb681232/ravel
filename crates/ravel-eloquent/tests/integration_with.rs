//! Integration test for `with()` eager-loading.

use ravel_eloquent::{Model, ModelExt, ModelMeta};
use sea_orm::ConnectionTrait;
use serde::{Deserialize, Serialize};

// ── Models ─────────────────────────────────────────────────────────────────

#[derive(Model, Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
#[model(table = "users")]
struct User {
    #[model(id)]
    pub id: i32,
    pub name: String,
    #[model(has_many, table = "posts")]
    pub posts: ravel_eloquent::HasMany<Post>,
}

#[derive(Model, Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
#[model(table = "posts")]
struct Post {
    #[model(id)]
    pub id: i32,
    pub title: String,
    pub user_id: i32,
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[test]
fn test_model_meta_has_relations() {
    let rels = User::relations();
    assert_eq!(rels.len(), 1);
    assert_eq!(rels[0].field_name, "posts");
    assert_eq!(rels[0].table_name, "posts");
    assert_eq!(rels[0].foreign_key, "user_id");
    assert_eq!(rels[0].local_key, "id");
}

#[test]
fn test_user_get_relation() {
    let rel = User::get_relation("posts");
    assert!(rel.is_some());
    let rel = rel.unwrap();
    assert_eq!(rel.field_name, "posts");
}

#[test]
fn test_user_get_relation_missing() {
    let rel = User::get_relation("comments");
    assert!(rel.is_none());
}

#[test]
fn test_post_columns_from_callback() {
    let rel = User::get_relation("posts").unwrap();
    let cols = (rel.get_columns)();
    assert!(cols.contains(&"id"));
    assert!(cols.contains(&"title"));
    assert!(cols.contains(&"user_id"));
}

#[tokio::test]
async fn test_all_with_eager_loads_posts() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();

    // Create tables
    db.execute_unprepared(
        "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
    )
    .await
    .unwrap();
    db.execute_unprepared(
        "CREATE TABLE posts (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, user_id INTEGER NOT NULL)",
    )
    .await
    .unwrap();

    // Seed data
    db.execute_unprepared("INSERT INTO users (name) VALUES ('Alice')")
        .await
        .unwrap();
    db.execute_unprepared("INSERT INTO posts (title, user_id) VALUES ('First Post', 1)")
        .await
        .unwrap();
    db.execute_unprepared("INSERT INTO posts (title, user_id) VALUES ('Second Post', 1)")
        .await
        .unwrap();

    // Eager load
    let users = User::all_with(&db, &["posts"]).await.unwrap();
    assert_eq!(users.len(), 1);
    let alice = &users[0];
    assert_eq!(alice.name, "Alice");
    assert_eq!(alice.posts.len(), 2);
    assert_eq!(alice.posts[0].title, "First Post");
    assert_eq!(alice.posts[1].title, "Second Post");
}

#[tokio::test]
async fn test_query_builder_with_eager_loads() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();

    db.execute_unprepared(
        "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
    )
    .await
    .unwrap();
    db.execute_unprepared(
        "CREATE TABLE posts (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, user_id INTEGER NOT NULL)",
    )
    .await
    .unwrap();

    db.execute_unprepared("INSERT INTO users (name) VALUES ('Bob')")
        .await
        .unwrap();
    db.execute_unprepared("INSERT INTO posts (title, user_id) VALUES ('Bob Post', 1)")
        .await
        .unwrap();

    let users: Vec<User> = User::query()
        .with("posts")
        .get(&db)
        .await
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].posts.len(), 1);
    assert_eq!(users[0].posts[0].title, "Bob Post");
}

#[tokio::test]
async fn test_all_without_with_does_not_load() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();

    db.execute_unprepared(
        "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
    )
    .await
    .unwrap();
    db.execute_unprepared(
        "CREATE TABLE posts (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, user_id INTEGER NOT NULL)",
    )
    .await
    .unwrap();

    db.execute_unprepared("INSERT INTO users (name) VALUES ('Charlie')")
        .await
        .unwrap();
    db.execute_unprepared("INSERT INTO posts (title, user_id) VALUES ('Charlie Post', 1)")
        .await
        .unwrap();

    let users = User::all(&db).await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].posts.len(), 0);
}

// ── Scope tests ─────────────────────────────────────────────────────

#[derive(Clone)]
struct StartsWithA;

impl ravel_eloquent::Scope<User> for StartsWithA {
    fn apply(self, qb: ravel_eloquent::QueryBuilder) -> ravel_eloquent::QueryBuilder {
        qb.where_str("name", "Alice")
    }
}

#[tokio::test]
async fn test_scope_filters_users() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
    ).await.unwrap();

    db.execute_unprepared("INSERT INTO users (name) VALUES ('Alice')").await.unwrap();
    db.execute_unprepared("INSERT INTO users (name) VALUES ('Bob')").await.unwrap();

    let users: Vec<User> = User::query()
        .scope(StartsWithA)
        .get(&db)
        .await
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");
}

// ── whereHas tests ───────────────────────────────────────────────────

#[tokio::test]
async fn test_where_has_filters_by_relation() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
    ).await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE posts (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, user_id INTEGER NOT NULL)",
    ).await.unwrap();

    db.execute_unprepared("INSERT INTO users (name) VALUES ('Alice')").await.unwrap();
    db.execute_unprepared("INSERT INTO users (name) VALUES ('Bob')").await.unwrap();
    db.execute_unprepared("INSERT INTO posts (title, user_id) VALUES ('First Post', 1)").await.unwrap();
    db.execute_unprepared("INSERT INTO posts (title, user_id) VALUES ('Second Post', 2)").await.unwrap();

    // Both Alice and Bob have posts
    let users: Vec<User> = User::query()
        .where_has::<User>("posts", |qb| qb)
        .get(&db)
        .await
        .unwrap();
    assert_eq!(users.len(), 2);

    // Filter: users who have a post titled "First Post"
    let users: Vec<User> = User::query()
        .where_has::<User>("posts", |qb| {
            qb.where_str("title", "First Post")
        })
        .get(&db)
        .await
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    // No user has a post titled "Nonexistent"
    let users: Vec<User> = User::query()
        .where_has::<User>("posts", |qb| {
            qb.where_str("title", "Nonexistent")
        })
        .get(&db)
        .await
        .unwrap();
    assert!(users.is_empty());
}
