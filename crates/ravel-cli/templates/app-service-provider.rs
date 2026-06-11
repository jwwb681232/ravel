use ravel_core::app::ServiceProvider;
use ravel_core::container::Container;
use anyhow::Result;

pub struct AppServiceProvider;

impl ServiceProvider for AppServiceProvider {
    fn register(&self, _container: &Container) -> Result<()> {
        // Register services here (Queue, Storage, etc.)
        Ok(())
    }

    fn name(&self) -> &str {
        "AppServiceProvider"
    }
}
