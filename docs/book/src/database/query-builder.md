# Query Builder

Ravel uses **SeaORM** as its query builder. SeaORM provides a fluent, type-safe API for constructing SQL queries in Rust. This guide covers the essential CRUD operations.

## Defining an Entity

Entities are Rust structs annotated with SeaORM derive macros, mapping to database tables:

```rust
use ravel_db_seaorm::sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub email: String,
    pub age: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

## Select Queries

Build queries using the entity's `find()` method with filters, ordering, and limits:

```rust
use sea_orm::*;

// All users
let all = Users::find().all(&db).await?;

// Filtered
let adults = Users::find()
    .filter(users::Column::Age.gte(18))
    .all(&db).await?;

// With ordering and limit
let recent = Users::find()
    .order_by_desc(users::Column::Id)
    .limit(10)
    .all(&db).await?;

// Find by ID
let user = Users::find_by_id(42).one(&db).await?;
```

Compound filters:

```rust
let results = Users::find()
    .filter(
        Condition::all()
            .add(users::Column::Age.gte(18))
            .add(users::Column::Name.like("A%"))
    )
    .order_by_asc(users::Column::Name)
    .all(&db).await?;
```

## Insert

Create a new record using `ActiveModel`:

```rust
use sea_orm::ActiveValue::Set;

let user = users::ActiveModel {
    name: Set("Alice".to_owned()),
    email: Set("alice@example.com".to_owned()),
    age: Set(Some(30)),
    ..Default::default()
};

let result = user.insert(&db).await?;
println!("Inserted user ID: {}", result.id);
```

## Update

Load an existing record, modify it, and save:

```rust
let user = Users::find_by_id(42).one(&db).await?;
if let Some(u) = user {
    let mut active: users::ActiveModel = u.into();
    active.email = Set("newemail@example.com".to_owned());
    active.age = Set(Some(31));
    active.update(&db).await?;
}
```

Or update in bulk:

```rust
Users::update_many()
    .col_expr(users::Column::Age, Expr::value(0))
    .filter(users::Column::Age.is_null())
    .exec(&db).await?;
```

## Delete

```rust
// Delete by ID
let user = Users::find_by_id(42).one(&db).await?;
if let Some(u) = user {
    u.delete(&db).await?;
}

// Bulk delete
Users::delete_many()
    .filter(users::Column::Age.is_null())
    .exec(&db).await?;
```

## Using with the Container

For dependency injection, bind the database connection and entities into the container:

```rust
use ravel_core::container::Container;
use ravel_db_seaorm::connection::ConnectionManager;

let manager = ConnectionManager::from_config("config")?;
let db = manager.connect("default").await?;
container.instance(db);
```

Then resolve in service providers or controllers:

```rust
fn register(&self, container: &Container) -> anyhow::Result<()> {
    let db = container.resolve::<DatabaseConnection>();
    // Register repositories that use db ...
    Ok(())
}
```

## Joins

```rust
let results = Posts::find()
    .join(JoinType::InnerJoin, posts::Relation::User.def())
    .filter(users::Column::Name.eq("Alice"))
    .all(&db).await?;
```

## Full CRUD Example

```rust
use sea_orm::*;

async fn create_user(db: &DatabaseConnection, name: &str, email: &str) -> Result<users::Model> {
    users::ActiveModel {
        name: Set(name.to_owned()),
        email: Set(email.to_owned()),
        ..Default::default()
    }.insert(db).await.map_err(Into::into)
}

async fn get_user(db: &DatabaseConnection, id: i32) -> Result<Option<users::Model>> {
    Users::find_by_id(id).one(db).await.map_err(Into::into)
}

async fn update_email(db: &DatabaseConnection, id: i32, email: &str) -> Result<()> {
    let user = Users::find_by_id(id).one(db).await?;
    if let Some(u) = user {
        let mut active: users::ActiveModel = u.into();
        active.email = Set(email.to_owned());
        active.update(db).await?;
    }
    Ok(())
}

async fn delete_user(db: &DatabaseConnection, id: i32) -> Result<()> {
    let user = Users::find_by_id(id).one(db).await?;
    if let Some(u) = user {
        u.delete(db).await?;
    }
    Ok(())
}
```
