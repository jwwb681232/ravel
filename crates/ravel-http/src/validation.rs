//! Validation engine — declarative field-level validation rules.
//!
//! # Usage
//!
//! ```rust
//! use ravel_http::validation::{Rule, FieldRule, Validator};
//!
//! let validator = Validator::new(vec![
//!     FieldRule::new("name", vec![Rule::Required, Rule::Min(3)]),
//!     FieldRule::new("email", vec![Rule::Required, Rule::Email]),
//!     FieldRule::new("age", vec![Rule::Min(18), Rule::Max(150)]),
//! ]);
//!
//! // Validate a JSON value
//! let value = serde_json::json!({"name": "Alice", "email": "a@b.com", "age": 25});
//! assert!(validator.validate(&value).is_ok());
//! ```

use serde_json::Value;
use std::collections::HashMap;

/// A single validation rule applied to a field value.
#[derive(Debug, Clone)]
pub enum Rule {
    /// Field must be present and non-empty.
    Required,
    /// String length must be >= n.
    Min(usize),
    /// String length must be <= n.
    Max(usize),
    /// Must look like an email address.
    Email,
    /// Must match the given regex pattern.
    Regex(String),
    /// Must be one of the listed values.
    In(Vec<String>),
}

/// Validation rules for a single field.
#[derive(Debug, Clone)]
pub struct FieldRule {
    pub field: String,
    pub rules: Vec<Rule>,
}

impl FieldRule {
    pub fn new(field: impl Into<String>, rules: Vec<Rule>) -> Self {
        Self {
            field: field.into(),
            rules,
        }
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

    /// Validate `value` (typically the full JSON object) against all rules.
    ///
    /// Returns `Ok(())` if all rules pass, or `Err(errors)` with one
    /// error per failing rule.
    pub fn validate(&self, value: &Value) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        for field_rule in &self.rules {
            let field_val = value.as_object().and_then(|obj| obj.get(&field_rule.field));

            for rule in &field_rule.rules {
                if let Some(msg) = Self::check_rule(field_val, rule, &field_rule.field) {
                    errors.push(ValidationError {
                        field: field_rule.field.clone(),
                        message: msg,
                    });
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validate with custom error message overrides.
    ///
    /// Custom messages are keyed by `"{field}.{rule_name}"`, e.g.
    /// `"name.required"` or `"email.email"`.
    pub fn validate_with_messages(
        &self,
        value: &Value,
        custom: &HashMap<String, String>,
    ) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        for field_rule in &self.rules {
            let field_val = value.as_object().and_then(|obj| obj.get(&field_rule.field));

            for rule in &field_rule.rules {
                if let Some(msg) = Self::check_rule(field_val, rule, &field_rule.field) {
                    let key = format!("{}.{}", field_rule.field, rule_name(rule));
                    let final_msg = custom.get(&key).cloned().unwrap_or(msg);
                    errors.push(ValidationError {
                        field: field_rule.field.clone(),
                        message: final_msg,
                    });
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Check a single rule. Returns `Some(error_message)` if the rule fails.
    fn check_rule(field_val: Option<&Value>, rule: &Rule, field_name: &str) -> Option<String> {
        match rule {
            Rule::Required => match field_val {
                None | Some(Value::Null) => Some(format!("{field_name} is required")),
                Some(Value::String(s)) if s.trim().is_empty() => {
                    Some(format!("{field_name} is required"))
                }
                _ => None,
            },

            Rule::Min(n) => match field_val {
                Some(Value::String(s)) if s.len() < *n => {
                    Some(format!("{field_name} must be at least {n} characters"))
                }
                Some(Value::Number(n_val)) => {
                    if let Some(i) = n_val.as_u64() {
                        if (i as usize) < *n {
                            Some(format!("{field_name} must be at least {n}"))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None, // not applicable — skip
            },

            Rule::Max(n) => match field_val {
                Some(Value::String(s)) if s.len() > *n => {
                    Some(format!("{field_name} must not exceed {n} characters"))
                }
                Some(Value::Number(n_val)) => {
                    if let Some(i) = n_val.as_u64() {
                        if (i as usize) > *n {
                            Some(format!("{field_name} must not exceed {n}"))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            },

            Rule::Email => match field_val {
                Some(Value::String(s)) if !s.contains('@') || !s.contains('.') => {
                    Some(format!("{field_name} must be a valid email address"))
                }
                _ => None,
            },

            Rule::Regex(pattern) => match field_val {
                Some(Value::String(s)) => {
                    if let Ok(re) = regex::Regex::new(pattern) {
                        if !re.is_match(s) {
                            Some(format!("{field_name} format is invalid"))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            },

            Rule::In(allowed) => match field_val {
                Some(Value::String(s)) if !allowed.contains(s) => Some(format!(
                    "{field_name} must be one of: {}",
                    allowed.join(", ")
                )),
                _ => None,
            },
        }
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
        let val = json!({"name": "Alice"});
        assert!(v.validate(&val).is_ok());
    }

    #[test]
    fn test_required_fail_missing() {
        let v = Validator::new(vec![FieldRule::new("name", vec![Rule::Required])]);
        let val = json!({});
        let errs = v.validate(&val).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].field, "name");
    }

    #[test]
    fn test_required_fail_empty() {
        let v = Validator::new(vec![FieldRule::new("name", vec![Rule::Required])]);
        let val = json!({"name": ""});
        assert!(v.validate(&val).is_err());
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
        // name: Required fails; email: Required fails, Email skipped (field absent)
        assert_eq!(errs.len(), 2);
    }

    #[test]
    fn test_custom_messages() {
        let v = Validator::new(vec![FieldRule::new("name", vec![Rule::Required])]);
        let mut msgs = HashMap::new();
        msgs.insert("name.required".into(), "Please enter your name".into());
        let errs = v.validate_with_messages(&json!({}), &msgs).unwrap_err();
        assert_eq!(errs[0].message, "Please enter your name");
    }

    #[test]
    fn test_min_number() {
        let v = Validator::new(vec![FieldRule::new("age", vec![Rule::Min(18)])]);
        assert!(v.validate(&json!({"age": 17})).is_err());
        assert!(v.validate(&json!({"age": 18})).is_ok());
        assert!(v.validate(&json!({"age": 25})).is_ok());
    }
}
