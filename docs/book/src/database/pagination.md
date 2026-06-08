# Pagination

Ravel provides a pagination system built on top of SeaORM's `PaginatorTrait`. The `Paginator` struct wraps a query and returns typed `Page<T>` results with metadata.

## The Page Type

A `Page<T>` contains the items for the current page along with metadata:

| Field       | Type   | Description |
|-------------|--------|-------------|
| `items`     | `Vec<T>` | The items on this page |
| `current`   | `u64`  | 1-based current page number |
| `last`      | `u64`  | Total number of pages |
| `total`     | `u64`  | Total items across all pages |
| `per_page`  | `u64`  | Items per page |

## Basic Usage

```rust
use ravel_db_seaorm::pagination::Paginator;
use sea_orm::*;

let page: Page<users::Model> = Paginator::new(
    Users::find().order_by_asc(users::Column::Id)
)
.per_page(15)
.page(&db, 1)
.await?;

println!("Page {} of {}", page.current, page.last);
println!("Showing {} of {} users", page.items.len(), page.total);

for user in &page.items {
    println!("{}: {}", user.id, user.name);
}
```

## Navigation Methods

```rust
if page.has_more() {
    println!("There is a next page (page {})", page.current + 1);
}

if page.has_previous() {
    println!("There is a previous page (page {})", page.current - 1);
}

if page.is_empty() {
    println!("No results found");
}

let item_count = page.count();
```

## Custom Page Size

The default is **15 items per page** (matching Laravel's default). Override with `per_page()`:

```rust
let page = Paginator::new(Users::find())
    .per_page(50)
    .page(&db, 2)
    .await?;
```

## Simple Pagination Helper

For one-off queries, use the `simple` static method:

```rust
let page = Paginator::simple(Users::find(), &db, 1, 20).await?;
```

## Example: Paginated User List

```rust
use ravel_db_seaorm::pagination::Paginator;
use ravel_facades::Route;
use axum::extract::Query;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct PageParams {
    page: Option<u64>,
    per_page: Option<u64>,
}

async fn list_users(
    db: axum::extract::Extension<DatabaseConnection>,
    Query(params): Query<PageParams>,
) -> impl axum::response::IntoResponse {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(15);

    let result = Paginator::new(
        Users::find().order_by_asc(users::Column::Id)
    )
    .per_page(per_page)
    .page(&db, page)
    .await
    .unwrap();

    axum::Json(serde_json::json!({
        "data": result.items,
        "meta": {
            "current_page": result.current,
            "last_page": result.last,
            "total": result.total,
            "per_page": result.per_page,
            "has_more": result.has_more(),
        }
    }))
}
```

## Core Page Type

The `ravel-db-core` crate also defines a generic `Page<T>` type for use across backends:

```rust
use ravel_db_core::pagination::Page;

let page = Page::new(items, total, page_num, per_page);
assert_eq!(page.last_page(), 5);  // total 100, per_page 20
assert!(page.has_more());
assert_eq!(page.count(), 20);
```
