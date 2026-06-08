use anyhow::Result;

pub struct Hash;

impl Hash {
    pub fn make(_password: &str) -> Result<String> {
        unimplemented!()
    }
    pub fn check(_password: &str, _hash: &str) -> Result<bool> {
        unimplemented!()
    }
}
