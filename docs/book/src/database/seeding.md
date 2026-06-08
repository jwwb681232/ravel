# Seeding

Database seeders populate your database with test or starter data. Ravel provides a `ravel db:seed` CLI command that runs a user-defined seed binary.

## CLI Command

```bash
# Run all seeders
ravel db:seed
```

This delegates to `cargo run --bin seed`, executing the seed binary generated at project scaffold time (`src/bin/seed.rs`).

## Seeder Structure

A typical seed binary registers seeders and runs them:

```rust
// src/bin/seed.rs
use ravel_db_seaorm::connection::ConnectionManager;
use sea_orm::*;
use sea_orm::ActiveValue::Set;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let manager = ConnectionManager::from_config("config")?;
    let db = manager.connect("default").await?;

    seed_users(&db).await?;
    seed_posts(&db).await?;

    println!("Database seeded successfully!");
    Ok(())
}
```

## Seeding Users

```rust
async fn seed_users(db: &DatabaseConnection) -> anyhow::Result<()> {
    use sea_orm::ActiveValue::Set;

    let users = vec![
        users::ActiveModel {
            name: Set("Alice".to_owned()),
            email: Set("alice@example.com".to_owned()),
            age: Set(Some(30)),
            ..Default::default()
        },
        users::ActiveModel {
            name: Set("Bob".to_owned()),
            email: Set("bob@example.com".to_owned()),
            age: Set(Some(25)),
            ..Default::default()
        },
    ];

    for user in users {
        user.insert(db).await?;
    }

    println!("Seeded {} users", users.len());
    Ok(())
}
```

## Seeding Related Data

```rust
async fn seed_posts(db: &DatabaseConnection) -> anyhow::Result<()> {
    // Fetch the seeded users to use as authors
    let users = Users::find().all(db).await?;

    for user in &users {
        let post = posts::ActiveModel {
            title: Set(format!("Post by {}", user.name)),
            body: Set("This is a seeded post body.".to_owned()),
            user_id: Set(user.id),
            ..Default::default()
        };
        post.insert(db).await?;
    }

    println!("Seeded {} posts for {} users", users.len(), users.len());
    Ok(())
}
```

## Using in Tests

Seeders are also useful in integration tests for setting up known state:

```rust
#[tokio::test]
async fn test_list_posts() {
    // Arrange: seed data
    let db = setup_test_db().await;
    seed_users(&db).await.unwrap();
    seed_posts(&db).await.unwrap();

    // Act: query
    let posts = Posts::find().all(&db).await.unwrap();

    // Assert
    assert_eq!(posts.len(), 2);
    assert_eq!(posts[0].title, "Post by Alice");
}
```

## Resetting Before Seeding

For seeders that should run on a clean slate, truncate tables first:

```rust
async fn seed_users(db: &DatabaseConnection) -> anyhow::Result<()> {
    // Clear existing data
    Users::delete_many().exec(db).await?;

    // Insert seed data
    let alice = users::ActiveModel {
        name: Set("Alice".to_owned()),
        email: Set("alice@example.com".to_owned()),
        ..Default::default()
    };
    alice.insert(db).await?;
    Ok(())
}
```

## Using the Connection Manager from the Container

If your application registers `ConnectionManager` in the service container, resolve it in the seed binary:

```rust
use ravel_core::app::Application;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Application::new()
        .load_env(".")
        .load_config("config")
        .boot()?;

    let container = ravel_core::app::APP.get().unwrap().container();
    let manager = container.resolve::<ConnectionManager>();
    let db = manager.connect("default").await?;

    seed_users(&db).await?;
    Ok(())
}
```
