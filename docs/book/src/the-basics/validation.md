# Validation

Ravel provides two approaches to validation:

1. **Validator** — direct, programmatic validation
2. **FormRequest** — automatic JSON parsing + validation via Axum extractor

Both support the same ruleset and produce 422 JSON responses on failure.

## FormRequest (Recommended)

Derive `Deserialize` and implement `FormRequest`. The `Validated<T>` extractor handles parsing and validation:

```rust
use ravel_http::form_request::{FormRequest, Validated};
use ravel_http::validation::{FieldRule, Rule};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub name: String,
    pub email: String,
    pub password: String,
    pub bio: Option<String>,
    pub role_id: i32,
}

impl FormRequest for CreateUserRequest {
    fn rules() -> Vec<FieldRule> {
        vec![
            FieldRule::new("name", vec![Rule::Required, Rule::Min(3)]),
            FieldRule::new("email", vec![
                Rule::Required,
                Rule::Email,
                Rule::Unique { table: "users", column: "email", ignore_id: None },
            ]).bail_on_first(),
            FieldRule::new("password", vec![
                Rule::Required, Rule::Min(8), Rule::Confirmed,
            ]),
            FieldRule::new("bio", vec![Rule::Max(500)]).nullable(),
            FieldRule::new("role_id", vec![
                Rule::Required,
                Rule::Exists { table: "roles", column: "id" },
            ]),
        ]
    }
}

// In your handler:
async fn store(
    State(state): State<AppState>,
    Validated(req): Validated<CreateUserRequest>,
) -> Result<impl IntoResponse, RavelError> {
    // req is guaranteed valid — email is unique, role exists, password confirmed
    let user = User::create(serde_json::to_value(&req)?, &state.db).await?;
    Ok((StatusCode::CREATED, Json(user)))
}
```

When validation fails:
```json
{
    "message": "Validation failed",
    "errors": {
        "email": ["email has already been taken"],
        "name": ["name is required"]
    }
}
```

## Direct Validator

Use `Validator` directly for cases that don't fit the FormRequest pattern:

```rust
use ravel_http::validation::{Validator, FieldRule, Rule};

let validator = Validator::new(vec![
    FieldRule::new("email", vec![Rule::Required, Rule::Email]),
    FieldRule::new("age", vec![Rule::Min(18), Rule::Max(150)]),
]);

let input = serde_json::json!({"email": "a@b.com", "age": 25});
match validator.validate(&input) {
    Ok(()) => { /* valid */ },
    Err(errors) => { /* handle errors */ },
}
```

## Validation Rules

### Standard Rules

| Rule | Description |
|------|-------------|
| `Required` | Field must be present and non-empty |
| `Min(n)` | String length or number ≥ n |
| `Max(n)` | String length or number ≤ n |
| `Email` | Must contain `@` and `.` |
| `Regex(pattern)` | Must match the given regex |
| `In(values)` | Must be one of the listed values |

### Database Rules (require `has_async_rules()` → validate_async)

| Rule | Description |
|------|-------------|
| `Unique { table, column, ignore_id }` | Value must be unique; `ignore_id` excludes a record on update |
| `Exists { table, column }` | Value must exist in the referenced table |

### Field-Level Rules

| Rule | Description |
|------|-------------|
| `Confirmed` | `{field}` must equal `{field}_confirmation` |

## Field Modifiers

Chain these on `FieldRule`:

```rust
FieldRule::new("email", vec![...])
    .bail_on_first()   // Stop after first failing rule
    .nullable()        // Skip rules if value is null/empty/missing
    .sometimes()       // Skip rules if field is missing; validate when present
```

| Modifier | Behavior |
|----------|----------|
| `bail_on_first()` | Stop at first error for this field |
| `nullable()` | Empty / null / missing → skip all rules |
| `sometimes()` | Missing → skip; present → validate normally |

## Database Validation

Unique and Exists rules require a database connection. When `Validated<T>` detects async rules, it automatically uses `validate_async()`.

Your AppState must implement `HasDb`:

```rust
use ravel_http::form_request::HasDb;
use sea_orm::DatabaseConnection;

#[derive(Clone)]
struct AppState {
    db: DatabaseConnection,
}

impl HasDb for AppState {
    fn db(&self) -> &DatabaseConnection { &self.db }
}
```

## Custom Messages

```rust
fn messages() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("name.required".into(), "Please enter your name".into());
    m.insert("email.unique".into(), "This email is already registered".into());
    m
}
```

Key format: `"{field}.{rule_name}"` — e.g. `"email.required"`, `"email.unique"`, `"password.confirmed"`.

## Authorization

```rust
fn authorize(&self) -> bool {
    // Only admins can use this form:
    self.role == "admin"
}
```

Returns 403 Forbidden on failure.
