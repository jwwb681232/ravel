//! Validation engine — declarative field-level validation rules.
//!
//! # Usage
//!
//! ```rust
//! use ravel_http::validation::{Rule, FieldRule, Validator};
//!
//! let validator = Validator::new(vec![
//!     FieldRule::new("name", vec![Rule::Required, Rule::Min(3)]),
//!     FieldRule::new("email", vec![Rule::Required, Rule::Email,
//!         Rule::Unique { table: "users", column: "email", ignore_id: None }]),
//!     FieldRule::new("password", vec![Rule::Required, Rule::Min(8), Rule::Confirmed]),
//!     FieldRule::new("bio", vec![Rule::Max(500)]).nullable(),
//! ]);
//!
//! let value = serde_json::json!({"name": "Alice", "email": "a@b.com", "password": "secret123", "password_confirmation": "secret123"});
//! assert!(validator.validate(&value).is_ok());
//! ```

use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, Value};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// A single validation rule applied to a field value.
#[derive(Debug, Clone)]
pub enum Rule {
    /// Field must be present and non-empty.
    Required,
    /// String / number must be >= n.
    Min(usize),
    /// String / number must be <= n.
    Max(usize),
    /// Must look like an email address.
    Email,
    /// Must match the given regex pattern.
    Regex(String),
    /// Must be one of the listed values.
    In(Vec<String>),
    /// Value must be unique in the given table.column.
    /// Pass `ignore_id` to allow the current record on update.
    Unique {
        table: &'static str,
        column: &'static str,
        ignore_id: Option<i64>,
    },
    /// Value must exist in the given table.column.
    Exists {
        table: &'static str,
        column: &'static str,
    },
    /// Must match the `{field}_confirmation` field.
    Confirmed,
}

/// Validation rules for a single field.
#[derive(Debug, Clone)]
pub struct FieldRule {
    pub field: String,
    pub rules: Vec<Rule>,
    /// Stop after the first failing rule for this field (default: false).
    pub bail: bool,
    /// Skip all rules if the value is null / empty / missing (default: false).
    pub nullable: bool,
    /// Skip all rules if the field is not present (default: false).
    /// When the field IS present, all rules run normally.
    pub sometimes: bool,
}

impl FieldRule {
    pub fn new(field: impl Into<String>, rules: Vec<Rule>) -> Self {
        Self {
            field: field.into(),
            rules,
            bail: false,
            nullable: false,
            sometimes: false,
        }
    }

    /// Stop at the first failing rule for this field.
    pub fn bail_on_first(mut self) -> Self {
        self.bail = true;
        self
    }

    /// Null / empty / missing values skip all rules.
    pub fn nullable(mut self) -> Self {
        self.nullable = true;
        self
    }

    /// Missing values skip all rules; present values are validated normally.
    pub fn sometimes(mut self) -> Self {
        self.sometimes = true;
        self
    }
}

/// A validation error for a single field.
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

/// The validator — holds a set of field rules and runs them against a value.
pub struct Validator {
    rules: Vec<FieldRule>,
}

impl Validator {
    /// Create a validator from field rules.
    pub fn new(rules: Vec<FieldRule>) -> Self {
        Self { rules }
    }

    /// Returns true if any rule in this validator needs database access.
    pub fn has_async_rules(&self) -> bool {
        self.rules.iter().any(|fr| {
            fr.rules
                .iter()
                .any(|r| matches!(r, Rule::Unique { .. } | Rule::Exists { .. }))
        })
    }

    // ── Public API ───────────────────────────────────────────────────
    // Defined below in "Core engine" section.

    // ── Core engine ─────────────────────────────────────────────────────

    /// Validate `value` against all sync rules.  Unique / Exists are
    /// skipped — use [`validate_async`](Self::validate_async) for DB rules.
    pub fn validate(&self, value: &JsonValue) -> Result<(), Vec<ValidationError>> {
        self.run_sync(value, &HashMap::new(), false)
    }

    /// Sync validation with custom error messages.
    pub fn validate_with_messages(
        &self,
        value: &JsonValue,
        custom: &HashMap<String, String>,
    ) -> Result<(), Vec<ValidationError>> {
        self.run_sync(value, custom, true)
    }

    /// Async validation including database-dependent rules (Unique / Exists).
    pub async fn validate_async(
        &self,
        value: &JsonValue,
        db: &DatabaseConnection,
    ) -> Result<(), Vec<ValidationError>> {
        self.run_async(value, &HashMap::new(), db, false).await
    }

    /// Async validation with custom error messages.
    pub async fn validate_async_with_messages(
        &self,
        value: &JsonValue,
        custom: &HashMap<String, String>,
        db: &DatabaseConnection,
    ) -> Result<(), Vec<ValidationError>> {
        self.run_async(value, custom, db, true).await
    }

    fn run_sync(
        &self,
        value: &JsonValue,
        custom: &HashMap<String, String>,
        has_custom: bool,
    ) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        let obj = value.as_object();

        for field_rule in &self.rules {
            let field_val = obj.and_then(|o| o.get(&field_rule.field));

            if field_rule.sometimes && field_val.is_none() {
                continue;
            }
            if field_rule.nullable && is_empty(field_val) {
                continue;
            }

            for rule in &field_rule.rules {
                // Confirmed handled first
                if matches!(rule, Rule::Confirmed) {
                    Self::check_confirmed(
                        field_val, &field_rule.field, obj, custom,
                        has_custom, &mut errors,
                    );
                    if field_rule.bail {
                        break;
                    }
                    continue;
                }
                // Skip async rules in sync path
                if matches!(rule, Rule::Unique { .. } | Rule::Exists { .. }) {
                    continue;
                }
                if let Some(msg) =
                    Self::check_rule_sync(field_val, rule, &field_rule.field)
                {
                    let key = format!("{}.{}", field_rule.field, rule_name(rule));
                    errors.push(ValidationError {
                        field: field_rule.field.clone(),
                        message: if has_custom {
                            custom.get(&key).cloned().unwrap_or(msg)
                        } else {
                            msg
                        },
                    });
                    if field_rule.bail {
                        break;
                    }
                }
            }
        }

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    async fn run_async(
        &self,
        value: &JsonValue,
        custom: &HashMap<String, String>,
        db: &DatabaseConnection,
        has_custom: bool,
    ) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        let obj = value.as_object();

        for field_rule in &self.rules {
            let field_val = obj.and_then(|o| o.get(&field_rule.field));

            if field_rule.sometimes && field_val.is_none() {
                continue;
            }
            if field_rule.nullable && is_empty(field_val) {
                continue;
            }

            for rule in &field_rule.rules {
                // Confirmed
                if matches!(rule, Rule::Confirmed) {
                    Self::check_confirmed(
                        field_val, &field_rule.field, obj, custom,
                        has_custom, &mut errors,
                    );
                    if field_rule.bail {
                        break;
                    }
                    continue;
                }
                // Async rule
                if matches!(rule, Rule::Unique { .. } | Rule::Exists { .. }) {
                    match Self::check_rule_async(field_val, rule, &field_rule.field, db).await {
                        Err(db_msg) => {
                            let key = format!("{}.{}", field_rule.field, rule_name(rule));
                            errors.push(ValidationError {
                                field: field_rule.field.clone(),
                                message: if has_custom {
                                    custom.get(&key).cloned().unwrap_or(db_msg)
                                } else {
                                    db_msg
                                },
                            });
                            if field_rule.bail {
                                break;
                            }
                        }
                        Ok(()) => {}
                    }
                    continue;
                }
                // Sync rule
                if let Some(msg) =
                    Self::check_rule_sync(field_val, rule, &field_rule.field)
                {
                    let key = format!("{}.{}", field_rule.field, rule_name(rule));
                    errors.push(ValidationError {
                        field: field_rule.field.clone(),
                        message: if has_custom {
                            custom.get(&key).cloned().unwrap_or(msg)
                        } else {
                            msg
                        },
                    });
                    if field_rule.bail {
                        break;
                    }
                }
            }
        }

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    fn check_confirmed(
        field_val: Option<&JsonValue>,
        field_name: &str,
        obj: Option<&serde_json::Map<String, JsonValue>>,
        custom: &HashMap<String, String>,
        has_custom: bool,
        errors: &mut Vec<ValidationError>,
    ) {
        let original = field_val.and_then(|v| v.as_str()).unwrap_or("");
        let conf_key = format!("{}_confirmation", field_name);
        let confirmation = obj
            .and_then(|o| o.get(&conf_key))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if original != confirmation {
            let key = format!("{}.confirmed", field_name);
            let msg = if has_custom {
                custom
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| format!("{field_name} confirmation does not match"))
            } else {
                format!("{field_name} confirmation does not match")
            };
            errors.push(ValidationError {
                field: field_name.to_string(),
                message: msg,
            });
        }
    }

    // ── Sync rule checks ───────────────────────────────────────────────

    fn check_rule_sync(
        field_val: Option<&JsonValue>,
        rule: &Rule,
        field_name: &str,
    ) -> Option<String> {
        match rule {
            Rule::Required => {
                if is_empty(field_val) {
                    Some(format!("{field_name} is required"))
                } else {
                    None
                }
            }
            Rule::Min(n) => match field_val {
                Some(JsonValue::String(s)) if s.len() < *n => {
                    Some(format!("{field_name} must be at least {n} characters"))
                }
                Some(JsonValue::Number(num)) => {
                    num.as_u64()
                        .filter(|i| (*i as usize) < *n)
                        .map(|_| format!("{field_name} must be at least {n}"))
                }
                _ => None,
            },
            Rule::Max(n) => match field_val {
                Some(JsonValue::String(s)) if s.len() > *n => {
                    Some(format!("{field_name} must not exceed {n} characters"))
                }
                Some(JsonValue::Number(num)) => {
                    num.as_u64()
                        .filter(|i| (*i as usize) > *n)
                        .map(|_| format!("{field_name} must not exceed {n}"))
                }
                _ => None,
            },
            Rule::Email => match field_val {
                Some(JsonValue::String(s)) if !s.contains('@') || !s.contains('.') => {
                    Some(format!("{field_name} must be a valid email address"))
                }
                _ => None,
            },
            Rule::Regex(pattern) => match field_val {
                Some(JsonValue::String(s)) => {
                    regex::Regex::new(pattern)
                        .ok()
                        .filter(|re| !re.is_match(s))
                        .map(|_| format!("{field_name} format is invalid"))
                }
                _ => None,
            },
            Rule::In(allowed) => match field_val {
                Some(JsonValue::String(s)) if !allowed.contains(s) => {
                    Some(format!(
                        "{field_name} must be one of: {}",
                        allowed.join(", ")
                    ))
                }
                _ => None,
            },
            Rule::Confirmed => {
                // Handled by caller (needs parent object access)
                None
            }
            // Async rules — sync path returns None (handled by async check)
            Rule::Unique { .. } | Rule::Exists { .. } => None,
        }
    }

    // ── Async rule checks ───────────────────────────────────────────────

    async fn check_rule_async(
        field_val: Option<&JsonValue>,
        rule: &Rule,
        field_name: &str,
        db: &DatabaseConnection,
    ) -> Result<(), String> {
        match rule {
            Rule::Unique {
                table,
                column,
                ignore_id,
            } => {
                let val = field_val.and_then(extract_db_value);
                let Some(ref db_val) = val else {
                    // null / missing is not a uniqueness violation
                    return Ok(());
                };

                let backend = db.get_database_backend();
                let (sql, params) = if let Some(exclude_id) = ignore_id {
                    (
                        format!(
                            "SELECT COUNT(*) FROM \"{table}\" WHERE \"{column}\" = $1 AND \"id\" != $2"
                        ),
                        vec![db_val.clone(), Value::BigInt(Some(*exclude_id))],
                    )
                } else {
                    (
                        format!(
                            "SELECT COUNT(*) FROM \"{table}\" WHERE \"{column}\" = $1"
                        ),
                        vec![db_val.clone()],
                    )
                };

                let stmt = Statement::from_sql_and_values(backend, &sql, params);
                let rows = db
                    .query_all_raw(stmt)
                    .await
                    .map_err(|e| format!("Database error: {e}"))?;
                let count: i64 = rows
                    .first()
                    .and_then(|r| r.try_get_by_index::<i64>(0).ok())
                    .unwrap_or(0);

                if count > 0 {
                    Err(format!("{field_name} has already been taken"))
                } else {
                    Ok(())
                }
            }
            Rule::Exists { table, column } => {
                let val = field_val.and_then(extract_db_value);
                let Some(ref db_val) = val else {
                    // null / missing → not checked (fail? or skip?)
                    // Laravel's `exists` does NOT fail when empty — only
                    // when present and not found.  We follow that.
                    return Ok(());
                };

                let backend = db.get_database_backend();
                let sql = format!(
                    "SELECT COUNT(*) FROM \"{table}\" WHERE \"{column}\" = $1"
                );
                let stmt =
                    Statement::from_sql_and_values(backend, &sql, [db_val.clone()]);
                let rows = db
                    .query_all_raw(stmt)
                    .await
                    .map_err(|e| format!("Database error: {e}"))?;
                let count: i64 = rows
                    .first()
                    .and_then(|r| r.try_get_by_index::<i64>(0).ok())
                    .unwrap_or(0);

                if count == 0 {
                    Err(format!("The selected {field_name} is invalid"))
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn is_empty(v: Option<&JsonValue>) -> bool {
    match v {
        None | Some(JsonValue::Null) => true,
        Some(JsonValue::String(s)) => s.trim().is_empty(),
        _ => false,
    }
}

/// Extract a database-comparable value from a JSON value.
fn extract_db_value(v: &JsonValue) -> Option<Value> {
    match v {
        JsonValue::String(s) => Some(Value::String(Some(s.clone()))),
        JsonValue::Number(n) => n
            .as_i64()
            .map(|i| Value::BigInt(Some(i)))
            .or_else(|| n.as_f64().map(|f| Value::Double(Some(f)))),
        JsonValue::Bool(b) => Some(Value::Bool(Some(*b))),
        _ => None,
    }
}

/// Return the short name of a rule (for custom message keys).
fn rule_name(rule: &Rule) -> &'static str {
    match rule {
        Rule::Required => "required",
        Rule::Min(_) => "min",
        Rule::Max(_) => "max",
        Rule::Email => "email",
        Rule::Regex(_) => "regex",
        Rule::In(_) => "in",
        Rule::Unique { .. } => "unique",
        Rule::Exists { .. } => "exists",
        Rule::Confirmed => "confirmed",
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_required_pass() {
        let v = Validator::new(vec![FieldRule::new("name", vec![Rule::Required])]);
        assert!(v.validate(&json!({"name": "Alice"})).is_ok());
    }

    #[test]
    fn test_required_fail_missing() {
        let v = Validator::new(vec![FieldRule::new("name", vec![Rule::Required])]);
        let errs = v.validate(&json!({})).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].field, "name");
    }

    #[test]
    fn test_required_fail_empty() {
        let v = Validator::new(vec![FieldRule::new("name", vec![Rule::Required])]);
        assert!(v.validate(&json!({"name": ""})).is_err());
    }

    #[test]
    fn test_min_string() {
        let v = Validator::new(vec![FieldRule::new("name", vec![Rule::Min(3)])]);
        assert!(v.validate(&json!({"name": "Al"})).is_err());
        assert!(v.validate(&json!({"name": "Alice"})).is_ok());
    }

    #[test]
    fn test_max_string() {
        let v = Validator::new(vec![FieldRule::new("name", vec![Rule::Max(5)])]);
        assert!(v.validate(&json!({"name": "Alice"})).is_ok());
        assert!(v.validate(&json!({"name": "Alicia"})).is_err());
    }

    #[test]
    fn test_email() {
        let v = Validator::new(vec![FieldRule::new("email", vec![Rule::Email])]);
        assert!(v.validate(&json!({"email": "a@b.com"})).is_ok());
        assert!(v.validate(&json!({"email": "invalid"})).is_err());
    }

    #[test]
    fn test_in_rule() {
        let v = Validator::new(vec![FieldRule::new(
            "role",
            vec![Rule::In(vec!["admin".into(), "user".into()])],
        )]);
        assert!(v.validate(&json!({"role": "admin"})).is_ok());
        assert!(v.validate(&json!({"role": "guest"})).is_err());
    }

    #[test]
    fn test_multiple_errors() {
        let v = Validator::new(vec![
            FieldRule::new("name", vec![Rule::Required]),
            FieldRule::new("email", vec![Rule::Required, Rule::Email]),
        ]);
        let errs = v.validate(&json!({})).unwrap_err();
        assert_eq!(errs.len(), 2);
    }

    #[test]
    fn test_custom_messages() {
        let v = Validator::new(vec![FieldRule::new("name", vec![Rule::Required])]);
        let mut msgs = HashMap::new();
        msgs.insert("name.required".into(), "Please enter your name".into());
        let errs = v
            .validate_with_messages(&json!({}), &msgs)
            .unwrap_err();
        assert_eq!(errs[0].message, "Please enter your name");
    }

    #[test]
    fn test_min_number() {
        let v = Validator::new(vec![FieldRule::new("age", vec![Rule::Min(18)])]);
        assert!(v.validate(&json!({"age": 17})).is_err());
        assert!(v.validate(&json!({"age": 18})).is_ok());
        assert!(v.validate(&json!({"age": 25})).is_ok());
    }

    // ── New rule tests ──────────────────────────────────────────────

    #[test]
    fn test_nullable_skips_when_empty() {
        let v = Validator::new(
            vec![FieldRule::new("bio", vec![Rule::Max(5)]).nullable()],
        );
        assert!(v.validate(&json!({"bio": ""})).is_ok());
        assert!(v.validate(&json!({"bio": null})).is_ok());
        assert!(v.validate(&json!({})).is_ok());
        // Present non-empty still validated
        assert!(v.validate(&json!({"bio": "too long text"})).is_err());
    }

    #[test]
    fn test_sometimes_skips_when_missing() {
        let v = Validator::new(
            vec![FieldRule::new("avatar", vec![Rule::Required]).sometimes()],
        );
        assert!(v.validate(&json!({})).is_ok());
        // When present, rules apply
        assert!(v.validate(&json!({"avatar": ""})).is_err());
        assert!(v.validate(&json!({"avatar": "ok"})).is_ok());
    }

    #[test]
    fn test_bail_stops_on_first_error() {
        let v = Validator::new(vec![FieldRule::new(
            "email",
            vec![Rule::Required, Rule::Email],
        )
        .bail_on_first()]);
        let errs = v.validate(&json!({})).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("required"));
    }

    #[tokio::test]
    async fn test_unique_checks_db() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .unwrap();
        db.execute_unprepared(
            "CREATE TABLE test_users (id INTEGER PRIMARY KEY AUTOINCREMENT, email TEXT NOT NULL)",
        )
        .await
        .unwrap();

        let v = Validator::new(vec![FieldRule::new(
            "email",
            vec![Rule::Unique {
                table: "test_users",
                column: "email",
                ignore_id: None,
            }],
        )]);

        // No rows → unique
        assert!(v.validate_async(&json!({"email": "a@b.com"}), &db).await.is_ok());

        // Insert a row
        db.execute_unprepared("INSERT INTO test_users (email) VALUES ('a@b.com')")
            .await
            .unwrap();

        // Now duplicate → fails
        let errs = v
            .validate_async(&json!({"email": "a@b.com"}), &db)
            .await
            .unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("already been taken"));
    }

    #[tokio::test]
    async fn test_unique_with_ignore_id() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .unwrap();
        db.execute_unprepared(
            "CREATE TABLE test_users (id INTEGER PRIMARY KEY AUTOINCREMENT, email TEXT NOT NULL)",
        )
        .await
        .unwrap();
        db.execute_unprepared("INSERT INTO test_users (id, email) VALUES (1, 'a@b.com')")
            .await
            .unwrap();

        let v = Validator::new(vec![FieldRule::new(
            "email",
            vec![Rule::Unique {
                table: "test_users",
                column: "email",
                ignore_id: Some(1),
            }],
        )]);

        // Same email, but ignoring id=1 (i.e. updating own record) → ok
        assert!(v
            .validate_async(&json!({"email": "a@b.com"}), &db)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_exists_checks_db() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .unwrap();
        db.execute_unprepared(
            "CREATE TABLE roles (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
        )
        .await
        .unwrap();
        db.execute_unprepared("INSERT INTO roles (name) VALUES ('admin')")
            .await
            .unwrap();

        let v = Validator::new(vec![FieldRule::new(
            "role_id",
            vec![Rule::Exists {
                table: "roles",
                column: "id",
            }],
        )]);

        assert!(v
            .validate_async(&json!({"role_id": 1}), &db)
            .await
            .is_ok());
        let errs = v
            .validate_async(&json!({"role_id": 99}), &db)
            .await
            .unwrap_err();
        assert!(errs[0].message.contains("invalid"));
    }

    #[test]
    fn test_has_async_rules_detection() {
        let v = Validator::new(vec![FieldRule::new("email", vec![Rule::Required])]);
        assert!(!v.has_async_rules());

        let v = Validator::new(vec![FieldRule::new(
            "email",
            vec![Rule::Required, Rule::Unique {
                table: "users",
                column: "email",
                ignore_id: None,
            }],
        )]);
        assert!(v.has_async_rules());
    }
}
