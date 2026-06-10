//! Compile-time integration test for #[derive(Model)] proc-macro.
//!
//! These tests verify that the macro generates correct types and methods.
//! No database connection needed — purely compile-time checks.

use ravel_eloquent::{Fillable, Model, ModelExt, ModelMeta, Replicates, Serializes};
use serde::{Deserialize, Serialize};

// ── Test model ──────────────────────────────────────────────────────────────

#[derive(Model, Clone, Debug, Serialize, Deserialize)]
#[model(table = "users")]
struct User {
    #[model(id)]
    pub id: i32,

    pub name: String,

    #[model(hidden)]
    pub password: String,
}

// ── ModelMeta tests ────────────────────────────────────────────────────────

#[test]
fn test_table_name() {
    assert_eq!(User::table_name(), "users");
}

#[test]
fn test_id_column() {
    assert_eq!(User::id_column(), "id");
}

#[test]
fn test_columns() {
    let cols = User::columns();
    assert!(cols.contains(&"id"));
    assert!(cols.contains(&"name"));
    assert!(cols.contains(&"password"));
}

#[test]
fn test_public_columns_excludes_hidden() {
    let cols = User::public_columns();
    assert!(cols.contains(&"id"));
    assert!(cols.contains(&"name"));
    assert!(!cols.contains(&"password"));
}

// ── Column enum tests ──────────────────────────────────────────────────────

#[test]
fn test_column_enum_pascal_case() {
    assert_eq!(UserColumn::Id.as_str(), "id");
    assert_eq!(UserColumn::Name.as_str(), "name");
    assert_eq!(UserColumn::Password.as_str(), "password");
}

#[test]
fn test_column_enum_full_coverage() {
    // Verify all non-relation columns are represented
    assert_eq!(User::columns().len(), 3);
}

// ── to_public / Public struct tests ────────────────────────────────────────

#[test]
fn test_to_public_excludes_password() {
    let user = User {
        id: 42,
        name: "Alice".into(),
        password: "secret123".into(),
    };

    let public = user.to_public();
    assert_eq!(public.id, 42);
    assert_eq!(public.name, "Alice");
}

#[test]
fn test_user_public_serializable() {
    let public = UserPublic {
        id: 1,
        name: "Bob".into(),
    };
    let json = serde_json::to_string(&public).unwrap();
    assert!(json.contains("\"id\""));
    assert!(json.contains("\"name\""));
}

#[test]
fn test_to_json_includes_password() {
    let user = User {
        id: 1,
        name: "Carol".into(),
        password: "hunter2".into(),
    };

    let json = user.to_json();
    assert_eq!(json["id"], 1);
    assert_eq!(json["name"], "Carol");
    assert_eq!(json["password"], "hunter2");
}

#[test]
fn test_to_public_json_excludes_password() {
    let user = User {
        id: 1,
        name: "Carol".into(),
        password: "hunter2".into(),
    };

    let json = user.to_public_json();
    assert_eq!(json["id"], 1);
    assert_eq!(json["name"], "Carol");
    assert!(json.get("password").is_none());
}

// ── Query builder tests ────────────────────────────────────────────────────

#[test]
fn test_query_returns_query_builder() {
    // query() returns QueryBuilder<Entity> — just check it compiles and exists
    let _query = User::query();
}

#[test]
fn test_where_str_returns_query_builder() {
    // where_str(col, val) returns QueryBuilder
    let _query = User::where_str("name", "Alice");
}

// ── Setter tests ───────────────────────────────────────────────────────────

#[test]
fn test_set_name_works() {
    let user = User {
        id: 0,
        name: "old".into(),
        password: "pw".into(),
    };

    let updated = user.set_name("Alice");
    assert_eq!(updated.name, "Alice");
    assert_eq!(updated.id, 0);
    assert_eq!(updated.password, "pw");
}

// ── Replicate tests ────────────────────────────────────────────────────────

#[test]
fn test_replicate_works() {
    let user = User {
        id: 1,
        name: "Dave".into(),
        password: "pwd".into(),
    };

    let replica = user.replicate();
    assert_eq!(replica.name, "Dave");
    assert_eq!(replica.password, "pwd");
    // Replicate should have the same id too (it's a clone)
    assert_eq!(replica.id, 1);
}

// ── Fillable test ──────────────────────────────────────────────────────────

#[test]
fn test_fillable_impl() {
    // fill() should be available on User instances
    let user = User {
        id: 1,
        name: "Old".into(),
        password: "secret".into(),
    };

    let data = serde_json::json!({"name": "NewName"});
    let filled = user.fill(data);
    // fill sets field(s) from JSON — at minimum name should be updated
    assert_eq!(filled.name, "NewName");
    // Unchanged fields should keep their values
    assert_eq!(filled.id, 1);
    assert_eq!(filled.password, "secret");
}

// ── ModelExt compile checks ───────────────────────────────────────────────

#[test]
fn test_model_ext_is_implemented() {
    // Helper: verify that User: ModelExt (compile-time check)
    fn _assert_model_ext<T: ModelExt>() {}
    _assert_model_ext::<User>();
}

// ── Serializes compile check ───────────────────────────────────────────────

#[test]
fn test_serializes_is_implemented() {
    fn _assert_serializes<T: Serializes>() {}
    _assert_serializes::<User>();
}

// ── Replicates compile check ───────────────────────────────────────────────

#[test]
fn test_replicates_is_implemented() {
    fn _assert_replicates<T: Replicates>() {}
    _assert_replicates::<User>();
}

// ── UserPublic struct fields compile check ─────────────────────────────────

#[test]
fn test_userpublic_has_expected_fields() {
    // Verify the UserPublic struct has the right fields by construction
    let _ = UserPublic {
        id: 1,
        name: "test".into(),
    };
}
