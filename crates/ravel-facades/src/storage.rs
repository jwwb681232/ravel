use anyhow::Result;

pub struct Storage;

impl Storage {
    pub fn put(_path: &str, _contents: &[u8]) -> Result<()> {
        unimplemented!()
    }
    pub fn get(_path: &str) -> Result<Vec<u8>> {
        unimplemented!()
    }
    pub fn exists(_path: &str) -> bool {
        unimplemented!()
    }
    pub fn delete(_path: &str) -> Result<()> {
        unimplemented!()
    }
}
