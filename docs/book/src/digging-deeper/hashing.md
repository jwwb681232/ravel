# Hashing

Ravel provides bcrypt password hashing through the `Hash` facade. It uses a cost factor of
**12** and requires **zero setup** — there is no dependency on `Application` or the service
container.

> Unlike `Cache` and `Crypt`, the `Hash` facade is a pure static API. You can use it
> anywhere, any time.

## Basic Usage

### Hashing a Password

```rust
use ravel_facades::Hash;

let hashed = Hash::make("my-secret-password")?;
// Returns a bcrypt hash string, e.g. "$2b$12$..."
```

### Verifying a Password

```rust
let hashed = Hash::make("correct-horse-battery-staple")?;

assert!(Hash::check("correct-horse-battery-staple", &hashed)?);
assert!(!Hash::check("wrong-password", &hashed)?);
```

## Example: User Authentication

```rust
use ravel_facades::Hash;

async fn register_user(email: String, password: String) -> Result<(), Error> {
    let hashed = Hash::make(&password)?;
    User::create(email, &hashed).await?;
    Ok(())
}

async fn login_user(email: String, password: String) -> Result<(), Error> {
    let user = User::find_by_email(&email).await?;

    if !Hash::check(&password, &user.password_hash)? {
        return Err(Error::Unauthorized("Invalid credentials".into()));
    }

    Ok(())
}
```

## Example: Updating a Password

```rust
use ravel_facades::Hash;

async fn change_password(user_id: u32, current: String, new: String) -> Result<(), Error> {
    let user = User::find(user_id).await?;

    // Verify current password before allowing change
    if !Hash::check(&current, &user.password_hash)? {
        return Err(Error::Unauthorized("Current password is incorrect".into()));
    }

    let new_hash = Hash::make(&new)?;
    User::update_password(user_id, &new_hash).await?;

    Ok(())
}
```

## API Reference

| Method | Description |
|--------|-------------|
| `Hash::make(password)` | Hash a plaintext password using bcrypt with cost 12 |
| `Hash::check(password, hash)` | Verify a plaintext password against a bcrypt hash string |

## Configuration

The bcrypt cost factor is hardcoded to **12** in the current implementation — a good
balance between security and performance. Each hash is unique thanks to bcrypt's embedded
salt, so two calls to `Hash::make("password")` will produce different outputs.

Changing the cost factor is not yet configurable. If you need a different cost, the
underlying `bcrypt` crate is available as a transitive dependency.

## Error Handling

`Hash::make` fails if the password is empty or contains a null byte (bcrypt limitations).
`Hash::check` fails if the hash string is malformed or uses an unsupported format. Both
return `anyhow::Result`.
