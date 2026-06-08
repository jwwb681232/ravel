# Schema Builder

The Schema Builder provides a **programmatic table creation API** — define tables with a fluent Rust interface instead of writing raw SQL. It generates SQL for PostgreSQL, MySQL, and SQLite.

## Creating a Table

Use `Schema::create` with a closure that receives a `Blueprint`:

```rust
use ravel_db_core::schema::{Schema, DbDriver};

let blueprint = Schema::create("users", |table| {
    table.id();
    table.string("name").nullable(false);
    table.string("email").unique();
    table.integer("age").default("0");
    table.timestamps();
});

let sql = blueprint.to_sql(DbDriver::Postgres);
println!("{}", sql);
```

This generates:

```sql
CREATE TABLE "users" (
  "id" INTEGER NOT NULL PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  "name" VARCHAR(255) NOT NULL,
  "email" VARCHAR(255) UNIQUE,
  "age" INTEGER DEFAULT 0,
  "created_at" TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  "updated_at" TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
)
```

## Column Types

| Method       | SQL Type      | Notes |
|--------------|---------------|-------|
| `id()`       | `INTEGER`     | Auto-incrementing primary key |
| `string(n)`  | `VARCHAR(255)`| Variable-length string |
| `text(n)`    | `TEXT`        | Large text |
| `integer(n)` | `INTEGER`     | 32-bit integer |
| `bigint(n)`  | `BIGINT`      | 64-bit integer |
| `boolean(n)` | `BOOLEAN`     | Defaults to `false` |
| `float(n)`   | `REAL`        | Floating-point number |
| `datetime(n)`| `TIMESTAMP`   | Date and time |
| `json(n)`    | `JSONB` / `TEXT` | JSON (JSONB on Postgres, TEXT elsewhere) |
| `uuid(n)`    | `UUID`        | UUID values |
| `timestamps()` | —           | Adds `created_at` and `updated_at` (both `TIMESTAMP`) |

## Column Modifiers

Each column type returns a mutable `ColumnDef` reference for chaining modifiers:

| Modifier         | Description |
|------------------|-------------|
| `nullable(bool)` | Set `NOT NULL` when `false` |
| `unique()`       | Add `UNIQUE` constraint |
| `primary()`      | Set as primary key |
| `auto_increment()` | Enable auto-increment |
| `default(val)`   | Set a default value |

```rust
Schema::create("posts", |table| {
    table.id();
    table.string("title").nullable(false);
    table.text("body");
    table.integer("user_id").nullable(false);
    table.boolean("published").default("false");
    table.datetime("published_at").nullable(true);
    table.timestamps();
});
```

## Drop If Exists

Chain `.drop_if_exists()` to generate a `DROP TABLE IF EXISTS` statement before the `CREATE TABLE`:

```rust
let sql = Schema::create("tmp_table", |table| {
    table.id();
    table.string("name");
})
.drop_if_exists()
.to_sql(DbDriver::Sqlite);

// DROP TABLE IF EXISTS "tmp_table";
// CREATE TABLE "tmp_table" ( "id" INTEGER PRIMARY KEY, "name" VARCHAR(255) )
```

## Driver-Specific Output

The `to_sql(driver)` method adapts to each database:

```rust
let bp = Schema::create("items", |t| {
    t.id();
    t.json("metadata");
    t.uuid("external_id");
});

// Postgres: "id" INTEGER ... GENERATED ALWAYS AS IDENTITY, "metadata" JSONB, "external_id" UUID
println!("{}", bp.to_sql(DbDriver::Postgres));

// MySQL: `id` INTEGER ... AUTO_INCREMENT, `metadata` TEXT, `external_id` UUID
println!("{}", bp.to_sql(DbDriver::Mysql));

// Sqlite: "id" INTEGER ... (auto-increment implicit), "metadata" TEXT, "external_id" UUID
println!("{}", bp.to_sql(DbDriver::Sqlite));
```

## Using With Migrations

Integrate the schema builder into SeaORM migrations for a higher-level API:

```rust
use ravel_db_core::schema::{Schema, DbDriver};

async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    let blueprint = Schema::create("users", |t| {
        t.id();
        t.string("name").nullable(false);
        t.string("email").unique();
    });

    let sql = blueprint.to_sql(DbDriver::Postgres);
    // Execute SQL via SeaORM
    manager.exec_stmt(sql).await?;
    Ok(())
}
```

## Full Example

```rust
use ravel_db_core::schema::{Schema, DbDriver};

fn create_tables() -> Vec<String> {
    let users = Schema::create("users", |t| {
        t.id();
        t.string("name").nullable(false);
        t.string("email").unique().nullable(false);
        t.integer("age").default("0");
        t.timestamps();
    });

    let posts = Schema::create("posts", |t| {
        t.id();
        t.string("title").nullable(false);
        t.text("body");
        t.integer("user_id").nullable(false);
        t.boolean("published").default("false");
        t.datetime("published_at");
        t.timestamps();
    });

    vec![
        users.to_sql(DbDriver::Postgres),
        posts.to_sql(DbDriver::Postgres),
    ]
}
```
