# Service Providers

Service providers are the central place to configure your application. Every Ravel service — routing, database, caching, queue — is bootstrapped via a service provider.

## The ServiceProvider Trait

```rust
use ravel_core::app::ServiceProvider;
use ravel_core::container::Container;
use anyhow::Result;

pub trait ServiceProvider: Send + Sync {
    fn register(&self, container: &Container) -> Result<()>;
    fn boot(&self, container: &Container) -> Result<()> { Ok(()) }
    fn name(&self) -> &str { std::any::type_name::<Self>() }
}
```

## The Two-Phase Bootstrap

Ravel boots in two phases, matching Laravel's design:

### Phase 1: `register()`

All providers' `register()` methods are called **before** any `boot()` method. Use this phase to bind services into the container:

```rust
use ravel_core::app::ServiceProvider;
use ravel_core::container::Container;
use ravel_facades::Route;
use anyhow::Result;

struct AppServiceProvider;

impl ServiceProvider for AppServiceProvider {
    fn register(&self, container: &Container) -> Result<()> {
        // Bind services — don't use other providers' services yet
        container.singleton(|_| MyService::new());
        Ok(())
    }

    fn boot(&self, container: &Container) -> Result<()> {
        // Now all providers are registered — safe to use any service
        let my_service: Arc<MyService> = container.resolve().unwrap();
        my_service.initialize();
        Ok(())
    }

    fn name(&self) -> &str { "AppServiceProvider" }
}
```

### Phase 2: `boot()`

After all `register()` methods complete, every provider's `boot()` method is called. At this point, all services from all providers are available:

```rust
struct RouteServiceProvider;

impl ServiceProvider for RouteServiceProvider {
    fn register(&self, _c: &Container) -> Result<()> {
        // Routes reference handler functions, not container services.
        // It's safe to register routes during register().
        Route::get("/", || async { "Hello, Ravel!" });
        Ok(())
    }

    fn name(&self) -> &str { "RouteServiceProvider" }
}
```

## Registering Providers

```rust
use ravel_core::app::Application;

Application::new()
    .load_env(".")
    .load_config("config")
    .register_provider(AppServiceProvider)
    .register_provider(RouteServiceProvider)
    .register_provider(DatabaseServiceProvider)
    .boot()?;
```

Order matters: providers are called in the order they're registered.

## Generating Providers

Use the CLI to scaffold a new provider:

```bash
ravel make:provider PaymentService
```

This creates `app/Providers/PaymentService.rs`:

```rust
use ravel_core::app::ServiceProvider;
use ravel_core::container::Container;
use anyhow::Result;

pub struct PaymentService;

impl ServiceProvider for PaymentService {
    fn register(&self, container: &Container) -> Result<()> {
        Ok(())
    }

    fn boot(&self, container: &Container) -> Result<()> {
        Ok(())
    }

    fn name(&self) -> &str { "PaymentService" }
}
```
