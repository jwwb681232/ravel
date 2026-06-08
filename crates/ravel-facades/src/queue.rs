use anyhow::Result;

pub struct Queue;

impl Queue {
    pub fn dispatch<J: serde::Serialize>(_job: J) -> Result<()> {
        unimplemented!()
    }
    pub fn dispatch_later<J: serde::Serialize>(
        _job: J,
        _delay: chrono::Duration,
    ) -> Result<()> {
        unimplemented!()
    }
    pub fn pending() -> Result<usize> {
        unimplemented!()
    }
}
