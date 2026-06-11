use ravel_core::app::ServiceProvider;
use ravel_core::container::Container;
use ravel_facades::Queue;
use anyhow::Result;

pub struct AppServiceProvider;

impl ServiceProvider for AppServiceProvider {
    fn register(&self, _container: &Container) -> Result<()> {
        // Register Queue with in-memory driver
        Queue::memory();
        Ok(())
    }

    fn name(&self) -> &str {
        "AppServiceProvider"
    }
}
