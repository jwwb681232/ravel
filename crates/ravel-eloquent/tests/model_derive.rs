//! Integration test for #[derive(Model)] proc-macro.

use ravel_eloquent::{HasRelations, Model, RelatedModel};
use sea_orm::Value;

#[derive(Model, Debug, serde::Serialize, serde::Deserialize)]
#[model(table = "users")]
struct User {
    #[model(id)]
    id: i32,

    #[model(string, 255)]
    name: String,

    #[model(string, 255, unique)]
    email: String,

    #[model(hidden)]
    #[model(string, 255)]
    password: String,
}

#[test]
fn test_model_column_enum() {
    assert_eq!(UserColumn::id.as_str(), "id");
    assert_eq!(UserColumn::name.as_str(), "name");
    assert_eq!(UserColumn::email.as_str(), "email");
    assert_eq!(UserColumn::password.as_str(), "password");
}

#[test]
fn test_model_to_public_hides_password() {
    let user = User {
        id: 1,
        name: "Alice".into(),
        email: "alice@example.com".into(),
        password: "secret".into(),
    };

    let public = user.to_public();
    assert_eq!(public.id, 1);
    assert_eq!(public.name, "Alice");
    assert_eq!(public.email, "alice@example.com");
    // UserPublic should NOT have a password field (compile-time guarantee)
}

#[test]
fn test_model_query_select_sql() {
    let query = User::query();
    let sql = query.to_select_sql();
    assert!(sql.contains("users"));
    assert!(sql.contains("SELECT * FROM \"users\""));
}

#[test]
fn test_model_query_where() {
    let query = User::r#where("name", "Alice");
    let sql = query.to_select_sql();
    assert!(sql.contains("WHERE"));
    assert!(sql.contains("name"));
    assert!(sql.contains("Alice"));
}

#[test]
fn test_model_query_order_limit() {
    let query = User::r#where("active", true)
        .order_by("created_at", "DESC")
        .limit(10);

    let sql = query.to_select_sql();
    assert!(sql.contains("ORDER BY"));
    assert!(sql.contains("DESC"));
    assert!(sql.contains("LIMIT 10"));
}

#[test]
fn test_model_count_sql() {
    let query = User::r#where("active", true);
    let sql = query.to_count_sql();
    assert!(sql.contains("COUNT(*)"));
}

#[test]
fn test_model_delete_sql() {
    let query = User::r#where("id", 1);
    let sql = query.to_delete_sql();
    assert!(sql.contains("DELETE FROM"));
}

#[test]
fn test_model_has_many_relations() {
    let user = User {
        id: 1,
        name: "Alice".into(),
        email: "alice@example.com".into(),
        password: "secret".into(),
    };

    let posts_query = user.has_many::<serde_json::Value>("posts", "user_id", Value::Int(Some(1)));
    let sql = posts_query.to_sql();
    assert!(sql.contains("posts"));
    assert!(sql.contains("user_id"));
}
