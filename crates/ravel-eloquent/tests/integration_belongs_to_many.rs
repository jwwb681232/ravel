//! Integration tests for BelongsToMany relations (many-to-many via pivot tables).

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
    #[model(has_many, via = "role_user", table = "roles")]
    pub roles: ravel_eloquent::BelongsToMany<Role>,
}

#[derive(Model, Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
#[model(table = "roles")]
struct Role {
    #[model(id)]
    pub id: i32,
    pub name: String,
}

#[derive(Model, Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
#[model(table = "users")]
struct UserExplicit {
    #[model(id)]
    pub id: i32,
    pub name: String,
    #[model(has_many, via = "role_user", foreign_key = "user_id", related_key = "role_id", table = "roles")]
    pub roles: ravel_eloquent::BelongsToMany<Role>,
}

// ── Relation metadata tests ─────────────────────────────────────────────────

#[test]
fn test_belongs_to_many_relation_meta() {
    let rel = User::get_relation("roles").unwrap();
    assert_eq!(rel.field_name, "roles");
    assert_eq!(rel.table_name, "roles");
    assert_eq!(rel.kind, ravel_eloquent::RelationKind::BelongsToMany);
    assert!(rel.pivot_table.is_some());
    assert_eq!(rel.pivot_table.unwrap(), "role_user");
    assert!(rel.pivot_foreign_key.is_some());
    assert!(rel.pivot_related_key.is_some());
    // Default FK naming: user_id
    assert_eq!(rel.pivot_foreign_key.unwrap(), "user_id");
}

#[test]
fn test_belongs_to_many_explicit_keys() {
    let rel = UserExplicit::get_relation("roles").unwrap();
    assert_eq!(rel.pivot_foreign_key.unwrap(), "user_id");
    assert_eq!(rel.pivot_related_key.unwrap(), "role_id");
}

// ── Eager loading tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_belongs_to_many_eager_load() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();

    db.execute_unprepared(
        "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
    ).await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE roles (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
    ).await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE role_user (
            user_id INTEGER NOT NULL,
            role_id INTEGER NOT NULL,
            PRIMARY KEY (user_id, role_id)
        )",
    ).await.unwrap();

    // Seed
    db.execute_unprepared("INSERT INTO users (name) VALUES ('Alice')").await.unwrap();
    db.execute_unprepared("INSERT INTO users (name) VALUES ('Bob')").await.unwrap();
    db.execute_unprepared("INSERT INTO roles (name) VALUES ('Admin')").await.unwrap();
    db.execute_unprepared("INSERT INTO roles (name) VALUES ('Editor')").await.unwrap();
    db.execute_unprepared("INSERT INTO role_user (user_id, role_id) VALUES (1, 1)").await.unwrap();
    db.execute_unprepared("INSERT INTO role_user (user_id, role_id) VALUES (1, 2)").await.unwrap();
    db.execute_unprepared("INSERT INTO role_user (user_id, role_id) VALUES (2, 2)").await.unwrap();

    let users = User::all_with(&db, &["roles"]).await.unwrap();
    assert_eq!(users.len(), 2);

    let alice = users.iter().find(|u| u.name == "Alice").unwrap();
    assert_eq!(alice.roles.len(), 2);
    let role_names: Vec<&str> = alice.roles.iter().map(|r| r.name.as_str()).collect();
    assert!(role_names.contains(&"Admin"));
    assert!(role_names.contains(&"Editor"));

    let bob = users.iter().find(|u| u.name == "Bob").unwrap();
    assert_eq!(bob.roles.len(), 1);
    assert_eq!(bob.roles[0].name, "Editor");
}

#[tokio::test]
async fn test_belongs_to_many_empty_roles() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();

    db.execute_unprepared(
        "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
    ).await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE roles (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
    ).await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE role_user (
            user_id INTEGER NOT NULL,
            role_id INTEGER NOT NULL,
            PRIMARY KEY (user_id, role_id)
        )",
    ).await.unwrap();

    db.execute_unprepared("INSERT INTO users (name) VALUES ('NoRoles')").await.unwrap();

    let users = User::all_with(&db, &["roles"]).await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].roles.len(), 0);
}

#[tokio::test]
async fn test_belongs_to_many_with_query_builder() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();

    db.execute_unprepared(
        "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
    ).await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE roles (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
    ).await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE role_user (
            user_id INTEGER NOT NULL,
            role_id INTEGER NOT NULL,
            PRIMARY KEY (user_id, role_id)
        )",
    ).await.unwrap();

    db.execute_unprepared("INSERT INTO users (name) VALUES ('QUser')").await.unwrap();
    db.execute_unprepared("INSERT INTO roles (name) VALUES ('Admin')").await.unwrap();
    db.execute_unprepared("INSERT INTO role_user (user_id, role_id) VALUES (1, 1)").await.unwrap();

    let users: Vec<User> = User::query()
        .with("roles")
        .get(&db)
        .await
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].roles.len(), 1);
    assert_eq!(users[0].roles[0].name, "Admin");
}

// ── Pivot operations: attach / detach / sync ────────────────────────────────

#[tokio::test]
async fn test_attach_roles() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();

    db.execute_unprepared(
        "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
    ).await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE roles (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
    ).await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE role_user (
            user_id INTEGER NOT NULL,
            role_id INTEGER NOT NULL,
            PRIMARY KEY (user_id, role_id)
        )",
    ).await.unwrap();

    // Create user via INSERT
    db.execute_unprepared("INSERT INTO users (name) VALUES ('AttachUser')").await.unwrap();
    // Also create roles that actually exist
    db.execute_unprepared("INSERT INTO roles (name) VALUES ('Admin')").await.unwrap();
    db.execute_unprepared("INSERT INTO roles (name) VALUES ('Editor')").await.unwrap();
    let user = User::find(&db, 1).await.unwrap().unwrap();

    // Attach role ids 1 and 2
    user.attach("roles", &[1, 2], &db).await.unwrap();

    // Verify pivot table via eager load
    let users = User::all_with(&db, &["roles"]).await.unwrap();
    let u = users.iter().find(|u| u.name == "AttachUser").unwrap();
    assert_eq!(u.roles.len(), 2);
}

#[tokio::test]
async fn test_detach_roles() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();

    db.execute_unprepared(
        "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
    ).await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE roles (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
    ).await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE role_user (
            user_id INTEGER NOT NULL,
            role_id INTEGER NOT NULL,
            PRIMARY KEY (user_id, role_id)
        )",
    ).await.unwrap();

    db.execute_unprepared("INSERT INTO users (name) VALUES ('DetachUser')").await.unwrap();
    db.execute_unprepared("INSERT INTO roles (name) VALUES ('A')").await.unwrap();
    db.execute_unprepared("INSERT INTO roles (name) VALUES ('B')").await.unwrap();
    db.execute_unprepared("INSERT INTO role_user (user_id, role_id) VALUES (1, 1)").await.unwrap();
    db.execute_unprepared("INSERT INTO role_user (user_id, role_id) VALUES (1, 2)").await.unwrap();

    let user = User::find(&db, 1).await.unwrap().unwrap();
    let deleted = user.detach("roles", &[1], &db).await.unwrap();
    assert_eq!(deleted, 1);

    let users = User::all_with(&db, &["roles"]).await.unwrap();
    let u = users.iter().find(|u| u.name == "DetachUser").unwrap();
    assert_eq!(u.roles.len(), 1);
    assert_eq!(u.roles[0].name, "B");
}

#[tokio::test]
async fn test_sync_roles() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();

    db.execute_unprepared(
        "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
    ).await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE roles (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
    ).await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE role_user (
            user_id INTEGER NOT NULL,
            role_id INTEGER NOT NULL,
            PRIMARY KEY (user_id, role_id)
        )",
    ).await.unwrap();

    db.execute_unprepared("INSERT INTO users (name) VALUES ('SyncUser')").await.unwrap();
    db.execute_unprepared("INSERT INTO roles (name) VALUES ('Old')").await.unwrap();
    db.execute_unprepared("INSERT INTO roles (name) VALUES ('New')").await.unwrap();
    db.execute_unprepared("INSERT INTO role_user (user_id, role_id) VALUES (1, 1)").await.unwrap();

    let user = User::find(&db, 1).await.unwrap().unwrap();
    // sync: detach all, attach only the given set
    user.sync("roles", &[2], &db).await.unwrap();

    let users = User::all_with(&db, &["roles"]).await.unwrap();
    let u = users.iter().find(|u| u.name == "SyncUser").unwrap();
    assert_eq!(u.roles.len(), 1);
    assert_eq!(u.roles[0].name, "New");
}
