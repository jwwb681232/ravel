//! JWT (JSON Web Token) creation and verification.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ravel_http::jwt::Jwt;
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize)]
//! struct Claims { sub: String, exp: usize }
//!
//! let jwt = Jwt::new(b"your-256-bit-secret-key-here!");
//! let token = jwt.encode(&Claims { sub: "1".into(), exp: 9999999999 })?;
//! let claims: Claims = jwt.decode(&token)?;
//! ```

use anyhow::{Context, Result};
#[allow(unused_imports)]
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sha2::digest::KeyInit;

/// JWT helper — sign and verify HS256 tokens.
pub struct Jwt {
    key: Vec<u8>,
}

impl Jwt {
    /// Create a new JWT instance with the given secret key.
    pub fn new(key: impl AsRef<[u8]>) -> Self {
        Self {
            key: key.as_ref().to_vec(),
        }
    }

    /// Encode claims into a JWT string (header.payload.signature).
    pub fn encode<T: Serialize>(&self, claims: &T) -> Result<String> {
        let header = base64url(br#"{"alg":"HS256","typ":"JWT"}"#);
        let payload = base64url(&serde_json::to_vec(claims).context("Failed to serialize claims")?);
        let signature = self.sign(&header, &payload);
        Ok(format!("{header}.{payload}.{signature}"))
    }

    /// Decode and verify a JWT string, returning the claims.
    pub fn decode<T: serde::de::DeserializeOwned>(&self, token: &str) -> Result<T> {
        let parts: Vec<&str> = token.splitn(3, '.').collect();
        if parts.len() != 3 {
            anyhow::bail!("Invalid JWT format");
        }

        let (header, payload, sig) = (parts[0], parts[1], parts[2]);
        let expected = self.sign(header, payload);

        if !constant_time_eq(sig.as_bytes(), expected.as_bytes()) {
            anyhow::bail!("JWT signature verification failed");
        }

        let decoded = base64_decode(payload).context("Failed to decode JWT payload")?;
        serde_json::from_slice(&decoded).context("Failed to deserialize claims")
    }

    fn sign(&self, header: &str, payload: &str) -> String {
        use hmac::Mac;
        let mut mac = <hmac::Hmac<Sha256> as KeyInit>::new_from_slice(&self.key).expect("HMAC key");
        Mac::update(&mut mac, header.as_bytes());
        Mac::update(&mut mac, b".");
        Mac::update(&mut mac, payload.as_bytes());
        let result = Mac::finalize(mac);
        base64url(&result.into_bytes())
    }
}

fn base64url(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

fn base64_decode(encoded: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .context("Invalid base64")
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestClaims {
        sub: String,
        name: String,
        iat: u64,
    }

    #[test]
    fn test_encode_decode() {
        let jwt = Jwt::new(b"test-secret-32-bytes-long-key!");
        let claims = TestClaims {
            sub: "42".into(),
            name: "Alice".into(),
            iat: 1700000000,
        };
        let token = jwt.encode(&claims).unwrap();
        let decoded: TestClaims = jwt.decode(&token).unwrap();
        assert_eq!(decoded, claims);
    }

    #[test]
    fn test_invalid_token_fails() {
        let jwt = Jwt::new(b"test-secret-32-bytes-long-key!");
        assert!(jwt.decode::<TestClaims>("a.b.c").is_err());
        assert!(jwt.decode::<TestClaims>("not-a-token").is_err());
    }

    #[test]
    fn test_wrong_key_fails() {
        let a = Jwt::new(b"key-a-32-bytes-long-keyAAAAAA!");
        let b = Jwt::new(b"key-b-32-bytes-long-keyBBBBBB!");
        let token = a
            .encode(&TestClaims {
                sub: "1".into(),
                name: "X".into(),
                iat: 0,
            })
            .unwrap();
        assert!(b.decode::<TestClaims>(&token).is_err());
    }
}
