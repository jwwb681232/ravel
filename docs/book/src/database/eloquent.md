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

Use `#[derive(Model)]` on a struct. The `#[model(table = "...")]` container attribute sets the database table name. Each column field is annotated with `#[model(...)]` to declare its type and constraints. Relationship fields use `HasMany<T>` / `HasOne<T>` / `BelongsToMany<T>` types.

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
    pub posts: ravel_eloquent::HasMany<Post>,

    #[model(belongs_to, from = "team_id", to = "id")]
    pub team: ravel_eloquent::HasOne<Team>,

    #[model(has_many, via = "role_user")]
    pub roles: ravel_eloquent::BelongsToMany<Role>,
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
| `soft_deletes` | Enable soft delete (`deleted_at` column) |
| `soft_deletes = "deleted_on"` | Soft deletes with custom column name |

### Relationship Attributes

| Attribute | Example |
|-----------|---------|
| `has_many` | `#[model(has_many)] pub posts: HasMany<Post>` |
| `has_one` | `#[model(has_one)] pub profile: HasOne<Profile>` |
| `belongs_to, from = "fk", to = "pk"` | `#[model(belongs_to, from = "team_id", to = "id")] pub team: HasOne<Team>` |
| `has_many, via = "junction"` | `#[model(has_many, via = "role_user")] pub roles: BelongsToMany<Role>` (many-to-many) |
| `has_many, via = "...", foreign_key = "...", related_key = "..."` | Override pivot FK column names |
| `table = "tbl"` on a relation field | Override the related table name |

---

## Generated Code

For a model named `User`, `#[derive(Model)]` generates:

### `UserColumn` enum

Each database column becomes a PascalCase variant. Implements `ColumnTrait`, `IdenStatic`, `Iterable`, `FromStr`, and `EntityName`.

```rust
assert_eq!(UserColumn::Email.as_str(), "email");
let col: UserColumn = "email".parse().unwrap(); // FromStr
```

### `UserPublic` struct

A `#[derive(Debug, Clone, serde::Serialize)]` struct with all **non-hidden**, **non-relation** fields. This is the safe type for API responses.

### SeaORM Entity / ActiveModel types

The macro also generates internal SeaORM 2.0-compatible types:

| Type | Purpose |
|------|---------|
| `UserEntity` | Unit struct impl `EntityTrait` — represents the table |
| `UserActiveModel` | `ActiveValue<Value>`-wrapped fields for insert/update |
| `UserPrimaryKey` | PK enum impl `PrimaryKeyTrait` + `PrimaryKeyToColumn` |
| `UserRelation` | Dummy relation enum |

### Trait Implementations

| Trait | Provides |
|-------|----------|
| `ModelMeta` | `table_name()`, `columns()`, `id_column()`, `public_columns()`, `find_column()`, `soft_delete_column()` |
| `ModelExt` | `find()`, `find_or_fail()`, `all()`, `all_with_trashed()`, `all_only_trashed()`, `create()`, `destroy()` |
| `ActiveModelExt` | `save()`, `insert()`, `update()`, `delete()`, `force_delete()`, `restore()`, `refresh()` |
| `Fillable` | `fill()`, `set_<field>()` per-field setters |
| `Serializes` | `to_public()`, `to_json()`, `to_public_json()` |
| `Replicates` | `replicate()` |
| `HasTimestamps` | `touch()` |
| `FromQueryResult` | SeaORM's row-to-model deserialization |
| `ModelTrait` | SeaORM's `get()` / `try_set()` on column enum |

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
User::destroy(&db, 42).await?;
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

// ── Replicate (clone + retain id) ──
let dup = user.replicate();
let dup = dup.save(&db).await?;         // new row if id=0

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
| `delete()` | `delete(self, db) -> Result<()>` | Soft-delete if enabled, else hard DELETE. Consumes self |
| `force_delete()` | `force_delete(self, db) -> Result<()>` | Hard DELETE, bypassing soft deletes |
| `restore()` | `restore(self, db) -> Result<Self>` | Restore a soft-deleted record (sets `deleted_at = NULL`) |
| `refresh()` | `refresh(self, db) -> Result<Self>` | Re-fetch current row from database |
| `replicate()` | `replicate(&self) -> Self` | Clone the model |
| `touch()` | `touch(self, db) -> Result<Self>` | Only update `updated_at` |
| `fill()` | `fill(self, data: Value) -> Self` | Bulk-assign fields from JSON |
| `set_<field>()` | `set_name(self, val) -> Self` | Set a single field (chainable) |

---

## Soft Deletes

Add `soft_deletes` to your container attribute to enable soft-delete behavior. An implicit `deleted_at` column is added automatically (or use a custom column name).

```rust
#[derive(Model, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[model(table = "users", soft_deletes)]
struct User {
    #[model(id)] pub id: i32,
    pub name: String,
}

// all() → automatically filters to WHERE deleted_at IS NULL
let active = User::all(&db).await?;

// find() → also filters trashed records
let found = User::find(&db, 1).await?; // None if soft-deleted

// Include trashed records
let all = User::all_with_trashed(&db).await?;

// Only trashed records
let trashed = User::all_only_trashed(&db).await?;

// Soft delete
user.delete(&db).await?;  // SET deleted_at = NOW()

// Hard delete (bypass soft deletes)
user.force_delete(&db).await?;  // DELETE FROM

// Restore a trashed record
let restored = trashed.restore(&db).await?; // SET deleted_at = NULL

// query() automatically filters soft-deleted records
let results: Vec<User> = User::query()
    .where_str("name", "Alice")
    .get(&db).await?;

// Query with trashed records
let results: Vec<User> = User::query_with_trashed()
    .get(&db).await?;

// destroy() also soft-deletes when enabled
User::destroy(&db, [1, 2, 3]).await?; // UPDATE SET deleted_at = NOW()
```

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

`QueryBuilder` wraps SeaORM 2.0 / sea-query expressions. All methods are chainable.

### Query Entry

```rust
use ravel_eloquent::Model;

let qb = User::query();                        // SELECT * FROM "users"
let qb = User::query().where_str("active", true); // with WHERE

// With soft deletes enabled, query() automatically adds WHERE deleted_at IS NULL
let qb = User::query_with_trashed();           // includes soft-deleted records
```

### WHERE Clauses

```rust
use ravel_eloquent::UserColumn;

User::query()
    .where_eq(UserColumn::Name, "Alice")          // = 'Alice'
    .where_gt(UserColumn::Age, 18)             // > 18
    .where_gte(UserColumn::Age, 18)            // >= 18
    .where_lt(UserColumn::Age, 65)             // < 65
    .where_ne(UserColumn::Status, "deleted")   // != 'deleted'
    .where_like(UserColumn::Name, "%Ali%")     // LIKE
    .where_in(UserColumn::Id, &[1, 2, 3])      // IN (1,2,3)
    .where_null(UserColumn::DeletedAt)         // IS NULL
    .where_not_null(UserColumn::Email)         // IS NOT NULL
    .where_between(UserColumn::Age, 18, 65)    // BETWEEN 18 AND 65
    .get(&db).await?;
```

For string-based column names (dynamic queries, macro-generated code):

```rust
User::query()
    .where_str("name", "Alice")
    .where_str("age", 18)
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
User::query().count(&db).await?;                        // → u64
User::query().exists(&db).await?;                       // → bool
User::query().sum(UserColumn::Age, &db).await?;         // → f64
User::query().avg(UserColumn::Age, &db).await?;         // → f64
User::query().min(UserColumn::Id, &db).await?;          // → f64
User::query().max(UserColumn::Id, &db).await?;          // → f64
User::query().group_by(UserColumn::TeamId).get(&db).await?;
```

### Pagination

```rust
use ravel_eloquent::Page;

let page: Page<User> = User::query()
    .order_by_asc(UserColumn::Id)
    .paginate(&db, 1, 15).await?;

assert!(page.has_more());
for user in page.items {
    // ...
}
```

### Eager Loading — `.with()`

```rust
// Eager-load relations with the main query
let users: Vec<User> = User::query()
    .where_str("active", true)
    .with("posts")
    .with("roles")
    .get(&db).await?;

// Also available as a static method
let users = User::all_with(&db, &["posts", "roles"]).await?;

// Access eager-loaded data through the relation field
for user in &users {
    println!("{} has {} posts", user.name, user.posts.len());
    for post in user.posts.iter() {
        println!("  - {}", post.title);
    }
}
```

### Underlying SeaORM Access

Use `.into_select()` to get the raw `sea_query::SelectStatement` for advanced queries:

```rust
let select = User::query().into_select();
// Use any sea-query API directly
```

---

## Relationships

### Defining Relationships

```rust
#[derive(Model, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[model(table = "users")]
struct User {
    #[model(id)] pub id: i32,
    pub name: String,

    // One-to-Many: User has many Posts
    #[model(has_many)]
    pub posts: ravel_eloquent::HasMany<Post>,

    // One-to-One: User has one Profile
    #[model(has_one)]
    pub profile: ravel_eloquent::HasOne<Profile>,

    // Belongs-To: User belongs to a Team
    #[model(belongs_to, from = "team_id", to = "id")]
    pub team: ravel_eloquent::HasOne<Team>,

    // Many-to-Many: User has many Roles via role_user pivot
    #[model(has_many, via = "role_user")]
    pub roles: ravel_eloquent::BelongsToMany<Role>,
}
```

### Lazy Loading — `RelationQuery<R>`

Macro-generated relation methods return a `RelationQuery<R>` that supports filtering, ordering, and pagination:

```rust
let user = User::find(&db, 1).await?;

// user.posts() returns RelationQuery<Post>
let posts = user.posts()
    .where_eq(PostColumn::Published, true)
    .order_by_desc(PostColumn::CreatedAt)
    .limit(10)
    .get(&db).await?;

// user.team() returns RelationQuery<Team>
let team = user.team()
    .first(&db).await?;

// Aggregates on relations
let draft_count = user.posts()
    .where_eq(PostColumn::Published, false)
    .count(&db).await?;

let has_posts = user.posts().exists(&db).await?;
```

### Many-to-Many: Pivot Operations

```rust
let user = User::find(&db, 1).await?;

// Attach roles to this user
user.attach("roles", &[1, 2, 3], &db).await?;

// Detach specific roles
user.detach("roles", &[2], &db).await?;

// Sync: detach all, then attach only the given set
user.sync("roles", &[1, 3], &db).await?;
```

### RelationQuery Methods

| Method | Description |
|--------|-------------|
| `.where_eq(col, val)` | WHERE equality |
| `.where_gt(col, val)` | WHERE greater-than |
| `.where_in(col, vals)` | WHERE IN |
| `.where_null(col)` | WHERE IS NULL |
| `.where_not_null(col)` | WHERE IS NOT NULL |
| `.order_by_asc(col)` / `.order_by_desc(col)` | ORDER BY |
| `.limit(n)` / `.offset(n)` | Pagination |
| `.get(db)` | Fetch all matching |
| `.first(db)` | Fetch first match |
| `.count(db)` / `.exists(db)` | Aggregates |
| `.paginate(db, page, per_page)` | Paginated query |

---

## Scopes — Reusable Query Fragments

Scopes let you extract commonly-used query constraints into reusable types:

```rust
use ravel_eloquent::Scope;

#[derive(Clone)]
struct Active;

impl Scope<User> for Active {
    fn apply(self, qb: QueryBuilder) -> QueryBuilder {
        qb.where_eq(UserColumn::Status, "active")
    }
}

#[derive(Clone)]
struct Popular {
    min_posts: i32,
}

impl Scope<User> for Popular {
    fn apply(self, qb: QueryBuilder) -> QueryBuilder {
        qb.where_gt(UserColumn::PostCount, self.min_posts)
    }
}

// Chain scopes with normal query methods
let users = User::query()
    .scope(Active)
    .scope(Popular { min_posts: 10 })
    .order_by_desc(UserColumn::CreatedAt)
    .get(&db).await?;
```

## whereHas — Filter by Related Records

`where_has` filters parent records based on the existence of matching related records:

```rust
// All users who have at least one post
let users = User::query()
    .where_has::<User>("posts", |qb| qb)
    .get(&db).await?;

// Users who have a published post
let users = User::query()
    .where_has::<User>("posts", |qb| {
        qb.where_eq(PostColumn::Published, true)
    })
    .get(&db).await?;

// OR variant
let users = User::query()
    .where_has::<User>("posts", |qb| qb)
    .or_where_has::<User>("comments", |qb| qb)
    .get(&db).await?;
```

### whereHas Methods

| Method | Description |
|--------|-------------|
| `.where_has::<T>(rel, \|qb\| ...)` | Parent has related records matching filter |
| `.or_where_has::<T>(rel, \|qb\| ...)` | OR variant |

Supports `HasMany`, `HasOne`, and `BelongsTo` relations. BelongsToMany support is planned.

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
| `ModelMeta` | `table_name()`, `columns()`, `id_column()`, `public_columns()`, `find_column()`, `soft_delete_column()` | Metadata |
| `ModelExt` | `find()`, `find_or_fail()`, `all()`, `all_with_trashed()`, `all_only_trashed()`, `create()`, `destroy()` | Static CRUD |
| `ActiveModelExt` | `save()`, `insert()`, `update()`, `delete()`, `force_delete()`, `restore()`, `refresh()` | Instance writes |
| `Fillable` | `fill()`, `set_<field>()` | Mass assignment |
| `Serializes` | `to_public()`, `to_json()`, `to_public_json()` | JSON |
| `Replicates` | `replicate()` | Clone model |
| `HasTimestamps` | `touch()` | Update timestamps |
