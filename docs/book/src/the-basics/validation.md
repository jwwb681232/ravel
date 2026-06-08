# Validation

Ravel provides a validation engine for verifying incoming request data. You can use the `Validator` directly, or integrate with `FormRequest` for automatic validation and 422 error responses.

## Validation Rules

The following rules are available in `ravel_http::validation`:

| Rule | Description |
|------|-------------|
| `Rule::Required` | Field must be present and non-empty |
| `Rule::Min(n)` | String length or numeric value must be at least `n` |
| `Rule::Max(n)` | String length or numeric value must not exceed `n` |
| `Rule::Email` | Value must resemble an email address (contains `@` and `.`) |
| `Rule::Regex(pattern)` | Value must match the given regex |
| `Rule::In(values)` | Value must be one of the listed strings |

## Using the Validator Directly

For one-off validation, create a `Validator` with field rules and validate a JSON value:

```rust
use ravel_http::validation::{Rule, FieldRule, Validator};
use serde_json::json;

let validator = Validator::new(vec![
    FieldRule::new("username", vec![Rule::Required, Rule::Min(3), Rule::Max(50)]),
    FieldRule::new("email", vec![Rule::Required, Rule::Email]),
    FieldRule::new("age", vec![Rule::Min(18), Rule::Max(120)]),
    FieldRule::new("role", vec![Rule::In(vec!["user".into(), "admin".into()])]),
]);

let data = json!({
    "username": "alice",
    "email": "alice@example.com",
    "age": 25,
    "role": "admin",
});

match validator.validate(&data) {
    Ok(()) => println!("Valid!"),
    Err(errors) => {
        for e in &errors {
            println!("{}: {}", e.field, e.message);
        }
    }
}
```

### Custom Error Messages

Pass custom messages keyed by `"{field}.{rule}"`:

```rust
use std::collections::HashMap;

let mut custom_messages = HashMap::new();
custom_messages.insert("username.required".into(), "Please choose a username".into());
custom_messages.insert("email.email".into(), "Enter a valid email address".into());
custom_messages.insert("age.min".into(), "You must be at least 18 years old".into());

match validator.validate_with_messages(&data, &custom_messages) {
    Ok(()) => println!("Valid!"),
    Err(errors) => { /* uses custom messages */ }
}
```

## FormRequest Trait

For automatic validation in handlers, implement `FormRequest` on a deserializable struct and use the `Validated<T>` extractor:

```rust
use ravel_http::form_request::{FormRequest, Validated};
use ravel_http::validation::{FieldRule, Rule};
use ravel_http::error::RavelError;
use ravel_facades::Route;
use axum::response::IntoResponse;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct RegisterRequest {
    name: String,
    email: String,
    password: String,
}

impl FormRequest for RegisterRequest {
    fn rules() -> Vec<FieldRule> {
        vec![
            FieldRule::new("name", vec![Rule::Required, Rule::Min(2), Rule::Max(100)]),
            FieldRule::new("email", vec![Rule::Required, Rule::Email]),
            FieldRule::new("password", vec![
                Rule::Required,
                Rule::Min(8),
                Rule::Regex(r"[A-Z]".to_string()),
            ]),
        ]
    }

    fn messages() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("name.required".into(), "Your name is required".into());
        m.insert("email.email".into(), "Please provide a valid email".into());
        m.insert("password.min".into(), "Password must be at least 8 characters".into());
        m.insert("password.regex".into(), "Password must contain at least one uppercase letter".into());
        m
    }
}

async fn register(
    Validated(req): Validated<RegisterRequest>,
) -> Result<impl IntoResponse, RavelError> {
    // req is guaranteed valid here
    Ok(serde_json::json!({
        "message": format!("Account created for {}", req.name),
    }))
}

Route::post("/register", register);
```

When validation fails, the handler returns a 422 response automatically:

```json
{
    "message": "Validation failed",
    "errors": {
        "password": [
            "Password must contain at least one uppercase letter"
        ],
        "name": [
            "Your name is required"
        ]
    }
}
```

## The `authorize()` Method

Override `authorize()` on your `FormRequest` to add access control. Return `false` to reject the request with a 403 Forbidden response:

```rust
#[derive(Deserialize)]
struct AdminActionRequest {
    action: String,
}

impl FormRequest for AdminActionRequest {
    fn rules() -> Vec<FieldRule> {
        vec![FieldRule::new("action", vec![Rule::Required])]
    }

    fn authorize(&self) -> bool {
        // Only allow requests during business hours
        self.action != "delete-everything"
    }
}
```

## Combining Rules

Multiple rules for a single field are evaluated independently. All failures are collected and returned:

```rust
FieldRule::new("password", vec![
    Rule::Required,
    Rule::Min(8),
    Rule::Max(128),
    Rule::Regex(r"[a-z]".to_string()),
    Rule::Regex(r"[0-9]".to_string()),
])
```

## Complete Example: User Registration

```rust
use ravel_http::form_request::{FormRequest, Validated};
use ravel_http::validation::{FieldRule, Rule};
use ravel_http::error::RavelError;
use ravel_facades::Route;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
struct UserRegistration {
    username: String,
    email: String,
    password: String,
    confirm_password: String,
    age: u32,
}

impl FormRequest for UserRegistration {
    fn rules() -> Vec<FieldRule> {
        vec![
            FieldRule::new("username", vec![
                Rule::Required,
                Rule::Min(3),
                Rule::Max(30),
                Rule::Regex(r"^[a-zA-Z0-9_]+$".to_string()),
            ]),
            FieldRule::new("email", vec![Rule::Required, Rule::Email]),
            FieldRule::new("password", vec![
                Rule::Required,
                Rule::Min(8),
                Rule::Max(128),
            ]),
            FieldRule::new("age", vec![Rule::Min(13), Rule::Max(150)]),
        ]
    }

    fn messages() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("username.required".into(), "Choose a username".into());
        m.insert("username.regex".into(), "Username can only contain letters, numbers, and underscores".into());
        m.insert("age.min".into(), "You must be at least 13 years old".into());
        m
    }
}

async fn register_user(
    Validated(req): Validated<UserRegistration>,
) -> Result<impl IntoResponse, RavelError> {
    // Validate password confirmation separately
    if req.password != req.confirm_password {
        let mut errors = HashMap::new();
        errors.insert("confirm_password".into(), vec!["Passwords do not match".into()]);
        return Err(RavelError::validation_error(errors));
    }

    Ok((StatusCode::CREATED, serde_json::json!({
        "message": format!("Welcome, {}!", req.username),
    })))
}

Route::post("/auth/register", register_user);
```
