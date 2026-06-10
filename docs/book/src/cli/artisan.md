# Artisan Commands

The `ravel` CLI is the command-line interface for Ravel applications. It
scaffolds projects, generates code, runs migrations, and manages the
development server.

## Installation

```bash
cargo install ravel-cli
```

Verify the installation:

```bash
ravel --version
```

## Command Reference

### Project Commands

| Command                   | Description                        |
|---------------------------|------------------------------------|
| `ravel new <name>`        | Create a new Ravel project         |
| `ravel serve`             | Start the development server       |
| `ravel route:list`        | Display all registered routes      |
| `ravel key:generate`      | Generate an application key        |

### `ravel new`

Scaffolds a complete Ravel project with the standard directory structure:

```bash
ravel new MyApp
cd my_app
```

Creates the following layout:

```
my_app/
├── Cargo.toml
├── .env / .env.example
├── config/app.toml
├── src/main.rs
├── src/bin/migrate.rs
├── src/bin/seed.rs
├── app/Http/Controllers/
├── app/Http/Middleware/
├── app/Http/Requests/
├── app/Models/
├── app/Providers/
├── app/Services/
├── database/migrations/
├── database/seeders/
├── routes/
├── storage/
└── tests/
```

### `ravel serve`

Starts the development server. Reads `[server]` configuration from `config/app.toml`:

```toml
# config/app.toml
[server]
host = "127.0.0.1"
port = 3000
```

```bash
ravel serve
# 🚀 Starting server...
#    http://127.0.0.1:3000
#    Press Ctrl+C to stop
```

Warns if the port is already in use.

### `ravel route:list`

Scans the `routes/` directory and prints registered routes:

```bash
ravel route:list
```

Output:

```
📋 Registered Routes
--------------------------------------------------------------
  📄 routes/web.rs
     GET      /
     POST     /users
     DELETE   /users/{id}
```

For in-process introspection, use `ravel_facades::Route::list()` which returns
`Vec<RouteEntry>` — useful for health-check endpoints.

### `ravel key:generate`

Generates a cryptographically random 256-bit key (via `rand::thread_rng()`) encoded as base64:

```bash
ravel key:generate
```

Output:

```
base64:abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890
```

### make: Commands

Scaffold new source files with sensible boilerplate templates:

| Command                           | Creates                       |
|-----------------------------------|-------------------------------|
| `ravel make:controller <name>`    | `app/Http/Controllers/*.rs`   |
| `ravel make:middleware <name>`    | `app/Http/Middleware/*.rs`    |
| `ravel make:model <name>`         | `app/Models/*.rs`             |
| `ravel make:migration <name>`     | `database/migrations/*.rs`    |
| `ravel make:seeder <name>`        | `database/seeders/*.rs`       |
| `ravel make:provider <name>`      | `app/Providers/*.rs`          |
| `ravel make:request <name>`       | `app/Http/Requests/*.rs`      |
| `ravel make:job <name>`           | `app/Jobs/*.rs`               |

```bash
# Scaffold a controller
ravel make:controller UserController

# Scaffold a model with a migration
ravel make:model User --migration

# Scaffold a job
ravel make:job SendWelcomeEmail
```

Model names can be any CamelCase string. The `--migration` / `-m` flag on
`make:model` generates a matching timestamped migration file in
`database/migrations/`.

### Migration Commands

| Command               | Description                           |
|-----------------------|---------------------------------------|
| `ravel migrate`       | Run pending migrations                |
| `ravel migrate:rollback` | Rollback the last migration       |
| `ravel migrate:refresh`  | Rollback all migrations, then re-apply |
| `ravel migrate:fresh`    | Drop all tables, then re-apply      |
| `ravel migrate:status`   | Show migration status                |

```bash
# Run all pending migrations
ravel migrate

# Rollback the last 3 migrations
ravel migrate:rollback --steps 3

# Drop everything and re-apply
ravel migrate:fresh

# Check which migrations have been applied
ravel migrate:status
```

The `--steps` flag accepts an optional number for `migrate` and
`migrate:rollback` to limit how many are run.

### db:seed

Run all registered database seeders:

```bash
ravel db:seed
```

Seeders populate your database with test or reference data. Add new seeders in
`database/seeders/` and invoke them from the seed binary.

## Example: Full Workflow

```bash
# 1. Create a new project
ravel new BlogApp
cd BlogApp

# 2. Generate an application key
ravel key:generate
# Add the output to .env as APP_KEY

# 3. Create a database migration
ravel make:migration CreatePostsTable

# Edit database/migrations/*_create_posts_table.rs to define columns

# 4. Create a model
ravel make:model Post --migration

# 5. Create a controller
ravel make:controller PostController

# Edit app/Http/Controllers/PostController.rs with your handlers

# 6. Run migrations
ravel migrate

# 7. Seed the database
ravel db:seed

# 8. Start the dev server
ravel serve
# Open http://127.0.0.1:3000
```

For a full list of options, use the `--help` flag on any command:

```bash
ravel --help
ravel make:controller --help
```
