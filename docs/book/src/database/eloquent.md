# Eloquent ORM

## Introduction

**ravel-eloquent** is the Active Record ORM for Ravel, modelled after Laravel Eloquent. It wraps SeaORM with a familiar, expressive API so you can define models, build queries, express relationships, and paginate results — all without writing raw SQL or deep SeaORM boilerplate.

The system is split across two crates:

| Crate | Purpose |
|-------|---------|
| `ravel-eloquent` | Runtime traits and types (`ModelQuery`, `Page`, `HasRelations`, re-export of SeaORM) |
| `ravel-eloquent-macros` | The `#[derive(Model)]` proc-macro that generates column enums, public views, and query entry points |

Add both to your `Cargo.toml`:

```toml
[dependencies]
ravel-eloquent = "0.1"
serde = { version = "1", features = ["derive"] }
```

The macros crate is pulled in automatically — you only need `ravel-eloquent` on your dependency list.

---

## Defining a Model

Use `#[derive(Model)]` on a struct. The `#[model(table = "...")]` container attribute sets the database table name. Every field is annotated with `#[model(...)]` to declare its column type and constraints.

```rust
use ravel_eloquent::Model;

#[derive(Model, serde::Serialize, serde::Deserialize)]
#[model(table = "users")]
struct User {
    #[model(id)] id: i32,
    #[model(string, 255)] name: String,
    #[model(string, 255, unique)] email: String,
    #[model(hidden)] password: String,
}
```

Multiple `#[model(...)]` attributes can be stacked on a single field:

```rust
#[derive(Model)]
#[model(table = "posts")]
struct Post {
    #[model(id)] id: i32,
    #[model(string, 255)] title: String,
    #[model(text)] body: String,
    #[model(hidden)]
    #[model(string, 255)]
    draft_token: String,
}
```

### Column Attributes Reference

| Attribute | Description |
|-----------|-------------|
| `id` | Primary key, auto-increment |
| `string, N` | `VARCHAR(N)` — defaults to 255 if N is omitted |
| `text` | Unlimited-length text |
| `integer` | 32-bit signed integer |
| `bigint` | 64-bit signed integer |
| `boolean` | Boolean (true / false) |
| `float` | Floating-point number |
| `datetime` | Date-time timestamp |
| `json` | Structured JSON data |
| `uuid` | Universally unique identifier |
| `hidden` | Excluded from the generated public view (see below) |
| `unique` | Adds a `UNIQUE` constraint |
| `nullable` | Allows `NULL` values |
| `timestamps` | Marks a field as `created_at` / `updated_at` |

---

## Generated Code

For a model named `User`, the `#[derive(Model)]` macro generates:

### `UserColumn` enum

Each field becomes a variant with a `as_str()` method that returns the column name:

```rust
let col = UserColumn::email;
assert_eq!(col.as_str(), "email");
```

### `UserPublic` struct

A serializable struct that mirrors `User` but **omits** any fields marked `#[model(hidden)]`. This is useful for API responses where sensitive data (passwords, tokens) must never leak.

### `to_public()` method

Converts the model into its public-safe counterpart:

```rust
let user = User { id: 1, name: "Alice".into(), email: "alice@example.com".into(), password: "secret".into() };
let public: UserPublic = user.to_public();
// public.password does not exist — compile-time guarantee
```

### `query()` static method

Returns a `ModelQuery<Self>` with the table name pre-configured:

```rust
let q: ModelQuery<User> = User::query();
assert!(q.to_select_sql().contains("SELECT * FROM \"users\""));
```

### `r#where(col, val)` static method

Shorthand for `User::query().r#where(col, val)`:

```rust
let q = User::r#where("email", "alice@example.com");
```

---

## ModelQuery Builder

`ModelQuery<T>` is the fluent query builder. Every method returns `Self` for chaining.

| Method | Description |
|--------|-------------|
| `.r#where("col", value)` | Add a `WHERE` equality clause |
| `.order_by("col", "ASC" \| "DESC")` | Add an `ORDER BY` clause |
| `.limit(n)` | Limit the result set to `n` rows |
| `.offset(n)` | Skip `n` rows before returning results |
| `.to_select_sql()` | Render the query as a `SELECT *` SQL string |
| `.to_count_sql()` | Render the query as a `SELECT COUNT(*)` SQL string |
| `.to_delete_sql()` | Render the query as a `DELETE` SQL string |

### Chaining example

```rust
use ravel_eloquent::Model;

#[derive(Model)]
#[model(table = "users")]
struct User {
    #[model(id)] id: i32,
    #[model(string, 255)] name: String,
    #[model(boolean)] active: bool,
}

let sql = User::r#where("active", true)
    .order_by("name", "ASC")
    .limit(25)
    .offset(0)
    .to_select_sql();

assert_eq!(
    sql,
    r#"SELECT * FROM "users" WHERE "active" = TRUE ORDER BY "name" ASC LIMIT 25 OFFSET 0"#
);
```

### Count query

```rust
let sql = User::r#where("active", true).to_count_sql();
// SELECT COUNT(*) as count FROM "users" WHERE "active" = TRUE
```

### Delete query

```rust
let sql = User::r#where("id", 42).to_delete_sql();
// DELETE FROM "users" WHERE "id" = 42
```

Values are quoted safely — strings are escaped, booleans become `TRUE` / `FALSE`, and integers are rendered as-is.

---

## Relationships

The `HasRelations` trait provides Eloquent-style relationship builders. It is automatically implemented for any type that implements `DeserializeOwned`.

### `has_many` — One-to-Many

```rust
use ravel_eloquent::{HasRelations, RelatedModel};
use sea_orm::Value;

let user = User {
    id: 1,
    name: "Alice".into(),
    email: "alice@example.com".into(),
};

let posts: RelationBuilder<Post> =
    user.has_many::<Post>("posts", "user_id", Value::Int(Some(user.id)));

let sql = posts.to_sql();
// SELECT * FROM "posts" WHERE "user_id" = 1
```

### `belongs_to` — Belongs-To

```rust
let post = Post { id: 10, user_id: 1, title: "Hello".into() };

let owner: RelationBuilder<User> =
    post.belongs_to::<User>("users", "id", Value::Int(Some(post.user_id)));

let sql = owner.to_sql();
// SELECT * FROM "users" WHERE "id" = 10
```

### `RelationBuilder<R>`

Both `has_many` and `belongs_to` return a `RelationBuilder<R>`, which exposes one method:

```rust
impl<R: RelatedModel> RelationBuilder<R> {
    pub fn to_sql(&self) -> String;
}
```

`RelatedModel` is a blanket trait implemented for any type that is `DeserializeOwned + Send + Sync + 'static` — you rarely need to implement it manually.

---

## Pagination

The `Page<T>` struct wraps a paginated result set:

```rust
use ravel_eloquent::Page;

let page = Page::<User> {
    items: vec![user],
    total: 42,
    page: 1,
    per_page: 15,
};

assert_eq!(page.last_page(), 3);  // ceil(42 / 15)
assert!(page.has_more());         // page 1 < 3
```

| Field / Method | Type | Description |
|----------------|------|-------------|
| `items` | `Vec<T>` | The records for the current page |
| `total` | `u64` | Total number of matching records across all pages |
| `page` | `u64` | Current page number (1-based) |
| `per_page` | `u64` | Number of records per page |
| `last_page()` | `u64` | Total number of pages (`total.div_ceil(per_page)`) |
| `has_more()` | `bool` | Whether there are more pages after the current one |

---

## Public / Hidden

The `#[model(hidden)]` attribute excludes a field from the generated `*Public` struct and its `to_public()` method. This is essential for sensitive fields that should never appear in API responses.

```rust
use ravel_eloquent::Model;

#[derive(Model, serde::Serialize, serde::Deserialize)]
#[model(table = "users")]
struct User {
    #[model(id)] pub id: i32,
    #[model(string, 255)] pub name: String,
    #[model(hidden)] pub password: String,  // excluded from UserPublic
}

let user = User { id: 1, name: "Alice".into(), password: "hunter2".into() };
let public = user.to_public();

// UserPublic is generated as:
//   struct UserPublic { pub id: i32, pub name: String }
// No password field exists — it cannot be serialised accidentally.
```

`UserPublic` derives `serde::Serialize` automatically, so it can be returned directly from HTTP handlers without risking exposure of hidden data.
