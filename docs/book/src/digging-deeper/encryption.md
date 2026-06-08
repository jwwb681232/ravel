# Encryption

Ravel provides AES-256-GCM symmetric encryption via the `Crypt` facade. It uses your
application's `APP_KEY` to encrypt and decrypt data, with a random 12-byte nonce generated
for every encryption operation.

> The Crypt facade requires the application key to be set via
> `Application::with_app_key()` before boot.

## Generating an Application Key

Use the `ravel key:generate` CLI command to produce a secure random 32-byte key encoded in
the `base64:...` format.

```bash
ravel key:generate
# Example output: base64:H4sIAAAA... (a 32-byte base64-encoded key)
```

Set this value as `APP_KEY` in your `.env` file:

```dotenv
APP_KEY=base64:H4sIAAAA...
```

## Enabling Encryption

Pass the key to `Application::with_app_key()`.

```rust
use ravel_core::app::Application;
use ravel_facades::Crypt;

Application::new()
    .load_env(".")
    .with_app_key(&std::env::var("APP_KEY").unwrap())?
    .register_provider(MyProvider)
    .boot()?;
```

## Basic Usage

### Encrypting and Decrypting Raw Bytes

```rust
use ravel_facades::Crypt;

let secret = b"SSN: 123-45-6789";

let encrypted = Crypt::encrypt(secret)?;
// encrypted is a base64 String with the nonce prepended

let decrypted: Vec<u8> = Crypt::decrypt(&encrypted)?;
assert_eq!(decrypted, secret);
```

### Encrypting Structured Values

`encrypt_value` / `decrypt_value` serialise to JSON before encrypting, making it easy to
store structured data.

```rust
use ravel_facades::Crypt;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct PaymentMethod {
    card_last_four: String,
    billing_zip: String,
}

let payment = PaymentMethod {
    card_last_four: "4242".into(),
    billing_zip: "94105".into(),
};

// Encrypt as JSON
let encrypted: String = Crypt::encrypt_value(&payment)?;

// Decrypt back
let decrypted: PaymentMethod = Crypt::decrypt_value(&encrypted)?;
```

## Example: Encrypting Sensitive Data in a Handler

```rust
use ravel_facades::{Crypt, Route};
use ravel_http::error::RavelError;

async fn store_payment_method(
    user_id: u32,
    card_number: String,
) -> Result<impl IntoResponse, RavelError> {
    // Encrypt before persisting
    let encrypted = Crypt::encrypt(card_number.as_bytes())
        .map_err(|e| RavelError::Internal(e.into()))?;

    // Store encrypted string in database
    save_encrypted_card(user_id, &encrypted).await?;

    Ok(response().json(serde_json::json!({ "status": "stored" }))?)
}

async fn get_payment_method(user_id: u32) -> Result<impl IntoResponse, RavelError> {
    let encrypted = get_encrypted_card(user_id).await?;
    let decrypted = Crypt::decrypt(&encrypted)
        .map_err(|e| RavelError::Internal(e.into()))?;

    Ok(response().json(serde_json::json!({
        "card_number": String::from_utf8_lossy(&decrypted)
    }))?)
}
```

## API Reference

| Method | Description |
|--------|-------------|
| `Crypt::encrypt(data)` | Encrypt raw bytes, returns base64-encoded string |
| `Crypt::decrypt(encoded)` | Decrypt base64-encoded ciphertext, returns `Vec<u8>` |
| `Crypt::encrypt_value(value)` | Serialise to JSON and encrypt |
| `Crypt::decrypt_value::<T>(enc)` | Decrypt and deserialise from JSON |

## Security Notes

- Each encryption call generates a **unique random 12-byte nonce**. Encrypting the same
  plaintext twice produces different ciphertexts.
- The nonce is prepended to the ciphertext and extracted automatically during decryption.
- The key must be exactly **32 bytes** (256 bits). The `base64:` prefix is stripped
  automatically by `from_key`.
- Use `encrypt_value` for structured data — it handles JSON serialisation and produces a
  single portable string.
