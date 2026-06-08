use anyhow::Result;

pub struct Crypt;

impl Crypt {
    pub fn encrypt(_plaintext: &[u8]) -> Result<String> {
        unimplemented!()
    }
    pub fn decrypt(_encoded: &str) -> Result<Vec<u8>> {
        unimplemented!()
    }
    pub fn encrypt_value<T: serde::Serialize>(_value: &T) -> Result<String> {
        unimplemented!()
    }
    pub fn decrypt_value<T: serde::de::DeserializeOwned>(_encoded: &str) -> Result<T> {
        unimplemented!()
    }
}
