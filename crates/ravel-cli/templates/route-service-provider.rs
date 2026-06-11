use ravel_core::app::ServiceProvider;
use ravel_core::container::Container;
use anyhow::Result;

pub struct RouteServiceProvider;

impl ServiceProvider for RouteServiceProvider {
    fn register(&self, _container: &Container) -> Result<()> {
        Ok(())
    }

    fn boot(&self, _container: &Container) -> Result<()> {
        crate::routes::web::register();
        Ok(())
    }

    fn name(&self) -> &str {
        "RouteServiceProvider"
    }
}
