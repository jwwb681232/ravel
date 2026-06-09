# Eloquent ORM

## Introduction

**ravel-eloquent** is the Active Record ORM for Ravel, modelled after Laravel Eloquent. It builds on SeaORM 2.0 with a familiar, expressive API so you can define models, write fluent queries, express relationships, and paginate results — all backed by SeaORM's type-safe engine.

The system is split across two crates:

| Crate | Purpose |
|-------|---------|
| `ravel-eloquent` | Runtime traits (`ModelMeta`, `ModelExt`, `ActiveModelExt`, `QueryBuilder`, `RelationQuery`, `Page`) |
| `ravel-eloquent-macros` | The `#[derive(Model)]` proc-macro that generates SeaORM entities, column enums, public views, and trait implementations |

Add to your `Cargo.toml`:

```toml
[dependencies]
ravel-eloquent = "0.1"
serde = { version = "1", features = ["derive"] }
```

The macros crate is pulled in automatically — you only need `ravel-eloquent` on your dependency list.

---

## Defining a Model

Use `#[derive(Model)]` on a struct. The `#[model(table = "...")]` container attribute sets the database table name. Each column field is annotated with `#[model(...)]` to declare its type and constraints. Relationship fields use `HasMany<T>` / `HasOne<T>` types.

```rust
use ravel_eloquent::Model;

#[derive(Model, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[model(table = "users", timestamps)]
struct User {
    #[model(id)]
    pub id: i32,

    pub name: String,                         // plain String → VARCHAR(255)

    #[model(string, 254, unique)]
    pub email: String,

    #[model(hidden)]
    pub password: String,                     // excluded from public view

    #[model(nullable, string, 500)]
    pub bio: Option<String>,

    #[model(integer)]
    pub team_id: i32,

    // Relations
    #[model(has_many)]
    pub posts: sea_orm::entity::prelude::HasMany<Post>,

    #[model(belongs_to, from = "team_id", to = "id")]
    pub team: sea_orm::entity::prelude::HasOne<Team>,
}
```

### Column Attributes Reference

| Attribute | Description |
|-----------|-------------|
| `id` | Primary key, auto-increment |
| `uuid` | Primary key, no auto-increment (UUID) |
| `string`, `string, N` | `VARCHAR(N)` — defaults to 255 if N is omitted |
| `text` | Unlimited-length text |
| `integer` | 32-bit signed integer |
| `bigint` | 64-bit signed integer |
| `boolean` | Boolean (true / false) |
| `float` | Floating-point number (f64) |
| `datetime` | Date-time timestamp |
| `json` | Structured JSON data |
| `hidden` | Excluded from `to_public()` output |
| `unique` | Adds a `UNIQUE` constraint |
| `nullable` | Allows `NULL` values (auto-detected for `Option<T>`) |
| `column = "real_name"` | Override the database column name |

### Container Attributes

| Attribute | Description |
|-----------|-------------|
| `table = "name"` | Database table name (**required**) |
| `timestamps` | Auto-add `created_at` / `updated_at` fields |

### Relationship Attributes

| Attribute | Example |
|-----------|---------|
| `has_many` | `#[model(has_many)] pub posts: HasMany<Post>` |
| `has_one` | `#[model(has_one)] pub profile: HasOne<Profile>` |
| `belongs_to, from = "fk", to = "pk"` | `#[model(belongs_to, from = "team_id", to = "id")] pub team: HasOne<Team>` |
| `has_many, via = "junction"` | `#[model(has_many, via = "role_user")] pub roles: HasMany<Role>` (many-to-many) |

---

## Generated Code

For a model named `User`, `#[derive(Model)]` generates:

### `UserColumn` enum

Each database column becomes a PascalCase variant with an `as_str()` method:

```rust
let col = UserColumn::Email;
assert_eq!(col.as_str(), "email");
```

### `UserPublic` struct

A `#[derive(Debug, Clone, serde::Serialize)]` struct with all **non-hidden**, **non-relation** fields. This is the safe type for API responses.

### Trait Implementations

| Trait | Provides |
|-------|----------|
| `ModelMeta` | `table_name()`, `columns()`, `id_column()`, `public_columns()` |
| `ModelExt` | `find()`, `find_or_fail()`, `all()`, `create()`, `delete_by_id()` |
| `ActiveModelExt` | `save()`, `insert()`, `update()`, `delete()`, `refresh()` |
| `Fillable` | `fill()`, `set_<field>()` per-field setters |
| `Serializes` | `to_public()`, `to_json()`, `to_public_json()` |
| `Replicates` | `replicate()` |
| `HasTimestamps` | `touch()` |

---

## Static CRUD

```rust
use ravel_eloquent::{Model, ModelExt};

#[derive(Model, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[model(table = "users")]
struct User {
    #[model(id)] pub id: i32,
    pub name: String,
    pub email: String,
}

// Find by primary key
let user = User::find(&db, 1).await?;         // Option<User>
let user = User::find_or_fail(&db, 1).await?;  // User (errors if missing)

// Fetch all
let users = User::all(&db).await?;             // Vec<User>

// Create from JSON
let user = User::create(
    serde_json::json!({"name": "Alice", "email": "alice@example.com"}),
    &db,
).await?;

// Delete by ID (no need to load the instance)
User::delete_by_id(&db, 42).await?;
```

---

## Instance Methods (Active Record)

All instance methods are **consumptive** (take `self`), returning a fresh instance. Use Rust variable shadowing for a fluent experience:

```rust
// ── Create ──
let user = User { id: 0, name: "Alice".into(), email: "a@e.com".into() };
let user = user.save(&db).await?;       // INSERT → id auto-filled

// ── Update ──
let user = user
    .set_name("Alice Updated")
    .set_email("alice@new.com")
    .save(&db).await?;                   // UPDATE

// ── Force INSERT / UPDATE ──
let user = user.insert(&db).await?;     // INSERT regardless of id
let user = user.update(&db).await?;     // UPDATE (errors if id == 0)

// ── Delete ──
user.delete(&db).await?;

// ── Refresh ──
let user = user.refresh(&db).await?;    // re-fetch from DB

// ── Touch (update updated_at only) ──
let user = user.touch(&db).await?;

// ── Replicate (clone + id = 0) ──
let dup = user.replicate();
let dup = dup.save(&db).await?;         // new row

// ── Bulk fill ──
let user = user.fill(serde_json::json!({
    "name": "Bob",
    "email": "bob@example.com"
}));
```

### Instance Method Reference

| Method | Signature | Behaviour |
|--------|-----------|-----------|
| `save()` | `save(self, db) -> Result<Self>` | INSERT if id==0, else UPDATE. Returns back-filled instance |
| `insert()` | `insert(self, db) -> Result<Self>` | Force INSERT, ignore current id |
| `update()` | `update(self, db) -> Result<Self>` | Force UPDATE, error if id==0 |
| `delete()` | `delete(self, db) -> Result<()>` | DELETE the row, consumes self |
| `refresh()` | `refresh(self, db) -> Result<Self>` | Re-fetch current row from database |
| `replicate()` | `replicate(&self) -> Self` | Clone with id reset to 0 |
| `touch()` | `touch(self, db) -> Result<Self>` | Only update `updated_at` |
| `fill()` | `fill(self, data: Value) -> Self` | Bulk-assign fields from JSON |
| `set_<field>()` | `set_name(self, val) -> Self` | Set a single field (chainable) |

---

## Serialization

```rust
// Full serialization (includes hidden fields)
let json = user.to_json();
// {"id":1,"name":"Alice","password":"secret",...}

// Safe serialization (excludes #[model(hidden)] fields)
let json = user.to_public_json();
// {"id":1,"name":"Alice",...}

// Get the Public struct directly
let public = user.to_public();
// UserPublic { id: 1, name: "Alice", ... }
```

---

## QueryBuilder

`QueryBuilder<E>` wraps SeaORM 2.0's type-safe `Select<E>`. All methods are chainable.

### Query Entry

```rust
use ravel_eloquent::Model;

let qb = User::query();                        // SELECT * FROM "users"
let qb = User::query().r#where("active", true); // with WHERE
```

### WHERE Clauses

```rust
use ravel_eloquent::UserColumn;

User::query()
    .filter(UserColumn::Name, "Alice")          // = 'Alice'
    .filter_gt(UserColumn::Age, 18)             // > 18
    .filter_gte(UserColumn::Age, 18)            // >= 18
    .filter_lt(UserColumn::Age, 65)             // < 65
    .filter_ne(UserColumn::Status, "deleted")   // != 'deleted'
    .filter_like(UserColumn::Name, "%Ali%")     // LIKE
    .filter_in(UserColumn::Id, &[1, 2, 3])      // IN (1,2,3)
    .filter_null(UserColumn::DeletedAt)         // IS NULL
    .filter_not_null(UserColumn::Email)         // IS NOT NULL
    .filter_between(UserColumn::Age, 18, 65)    // BETWEEN 18 AND 65
    .get(&db).await?;
```

For string-based column names (dynamic queries, macro-generated code):

```rust
User::query()
    .r#where("name", "Alice")
    .r#where("age", 18)
    .get(&db).await?;
```

### ORDER BY, LIMIT, OFFSET

```rust
User::query()
    .order_by_asc(UserColumn::Name)
    .order_by_desc(UserColumn::CreatedAt)
    .limit(15)
    .offset(30)
    .get(&db).await?;
```

### Aggregates

```rust
User::query().count(&db).await?;     // → u64
User::query().exists(&db).await?;    // → bool
```

### Pagination

```rust
use ravel_eloquent::Page;

let page: Page<User> = User::query()
    .order_by_asc(UserColumn::Id)
    .paginate(&db, 1, 15).await?;

assert!(page.has_more());
println!("Page {} of {}", page.page, page.last_page());
for user in page.items {
    // ...
}
```

### JOIN

```rust
use ravel_eloquent::QueryBuilder;
use sea_orm::JoinType;

// join related entities defined in the SeaORM relation model
User::query()
    .inner_join::<Post>(JoinType::InnerJoin)
    .get(&db).await?;
```

### Underlying SeaORM Access

Use `.into_select()` to get the raw SeaORM `Select<E>` for advanced queries:

```rust
use sea_orm::*;

let select: Select<User> = User::query().into_select();
// Use any SeaORM API directly
```

---

## Relationships

### Lazy Loading — `RelationQuery<R>`

Macro-generated relation methods return a `RelationQuery<R>` that supports filtering, ordering, and pagination:

```rust
let user = User::find(&db, 1).await?;

// user.posts() returns RelationQuery<Post>
let posts = user.posts()
    .filter(PostColumn::Published, true)
    .order_by_desc(PostColumn::CreatedAt)
    .limit(10)
    .get(&db).await?;

// user.team() returns RelationQuery<Team>
let team = user.team()
    .first(&db).await?;

// Aggregates on relations
let draft_count = user.posts()
    .filter(PostColumn::Published, false)
    .count(&db).await?;

let has_posts = user.posts().exists(&db).await?;
```

### RelationQuery Methods

| Method | Description |
|--------|-------------|
| `.filter(col, val)` | WHERE equality |
| `.filter_gt(col, val)` | WHERE greater-than |
| `.filter_in(col, vals)` | WHERE IN |
| `.filter_null(col)` | WHERE IS NULL |
| `.filter_not_null(col)` | WHERE IS NOT NULL |
| `.order_by_asc(col)` / `.order_by_desc(col)` | ORDER BY |
| `.limit(n)` / `.offset(n)` | Pagination |
| `.get(db)` | Fetch all matching |
| `.first(db)` | Fetch first match |
| `.count(db)` / `.exists(db)` | Aggregates |
| `.paginate(db, page, per_page)` | Paginated query |

---

## Traits at a Glance

All traits are auto-implemented by `#[derive(Model)]`. Use them as trait bounds when writing generic functions:

```rust
use ravel_eloquent::{
    ModelMeta, ModelExt, ActiveModelExt, Fillable, Serializes, Replicates,
};

async fn find_and_serialize<T>(db: &DatabaseConnection, id: i32) -> Result<Value>
where
    T: ModelExt + Serializes,
{
    let model = T::find_or_fail(db, id).await?;
    Ok(model.to_public_json())
}
```

| Trait | Key methods | Kind |
|-------|-------------|------|
| `ModelMeta` | `table_name()`, `columns()`, `id_column()`, `public_columns()` | Metadata |
| `ModelExt` | `find()`, `find_or_fail()`, `all()`, `create()`, `delete_by_id()` | Static CRUD |
| `ActiveModelExt` | `save()`, `insert()`, `update()`, `delete()`, `refresh()` | Instance writes |
| `Fillable` | `fill()`, `set_<field>()` | Mass assignment |
| `Serializes` | `to_public()`, `to_json()`, `to_public_json()` | JSON |
| `Replicates` | `replicate()` | Clone + reset id |
| `HasTimestamps` | `touch()` | Update timestamps |
