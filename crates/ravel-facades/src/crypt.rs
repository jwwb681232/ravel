//! Crypt facade — AES-256-GCM encryption.
use anyhow::Result;
use ravel_core::app::APP;
use ravel_core::crypt::Crypt as CryptEngine;

pub struct Crypt;

impl Crypt {
    fn engine() -> std::sync::Arc<CryptEngine> {
        let app = APP.get().expect("Application not booted");
        app.container()
            .resolve::<CryptEngine>()
            .expect("Crypt not registered — call Application::with_app_key(key) before boot")
    }

    pub fn encrypt(plaintext: &[u8]) -> Result<String> {
        Self::engine().encrypt(plaintext)
    }

    pub fn decrypt(encoded: &str) -> Result<Vec<u8>> {
        Self::engine().decrypt(encoded)
    }

    pub fn encrypt_value<T: serde::Serialize>(value: &T) -> Result<String> {
        Self::engine().encrypt_value(value)
    }

    pub fn decrypt_value<T: serde::de::DeserializeOwned>(encoded: &str) -> Result<T> {
        Self::engine().decrypt_value(encoded)
    }
}
