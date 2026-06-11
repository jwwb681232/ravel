use ravel_core::app::ServiceProvider;
use ravel_core::container::Container;
use anyhow::Result;

pub struct {{name}};

impl ServiceProvider for {{name}} {
    fn register(&self, container: &Container) -> Result<()> {
        // Bind services into the container here.
        let _ = container;
        Ok(())
    }

    fn boot(&self, container: &Container) -> Result<()> {
        // Wire up dependencies — all providers are registered at this point.
        let _ = container;
        Ok(())
    }

    fn name(&self) -> &str {
        "{{name}}"
    }
}
