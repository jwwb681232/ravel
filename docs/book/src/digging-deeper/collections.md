# Collections

Ravel's `Collection` type provides a fluent, chainable pipeline for working with
`Vec<T>` data. Inspired by Laravel's collection API, it lets you transform, filter, and
inspect iterables without nested loops.

## Creating a Collection

Use the `collect!()` macro to wrap any `Vec<T>` into a `Collection<T>`.

```rust
use ravel_facades::collect;

let nums = collect!(vec![1, 2, 3, 4, 5]);
```

## Pipeline Methods

All transformation methods consume `self` and return a new `Collection`, enabling
chaining.

```rust
use ravel_facades::collect;

let result = collect!(vec![1, 2, 3, 4, 5, 6])
    .filter(|n| n % 2 == 0)       // keep evens
    .map(|n| n * 10)              // multiply by 10
    .skip(1)                      // drop first
    .take(2)                      // keep next two
    .to_vec();                    // back to Vec

assert_eq!(result, vec![40, 60]);
```

### Sorting

For types implementing `Ord`:

```rust
let sorted = collect!(vec![3, 1, 4, 1, 5])
    .sort()
    .to_vec();

assert_eq!(sorted, vec![1, 1, 3, 4, 5]);
```

### Checking and Counting

```rust
let items = collect!(vec!["a", "b", "c"]);

assert!(items.contains(|&x| x == "b"));
assert!(!items.is_empty());
assert!(items.is_not_empty());
assert_eq!(items.count(), 3);
```

### First and Last

```rust
let items = collect!(vec![10, 20, 30]);

assert_eq!(items.first(), Some(&10));
assert_eq!(items.last(), Some(&30));
```

### Implode

Join display-able items into a string:

```rust
let result = collect!(vec!["apple", "banana", "cherry"]).implode(", ");
assert_eq!(result, "apple, banana, cherry");
```

### Each

Perform a side effect for every item without consuming the collection:

```rust
collect!(vec!["a", "b", "c"]).each(|s| {
    println!("Processing: {s}");
});
```

## Example: Filtering and Sorting User Data

```rust
use ravel_facades::collect;

struct User {
    name: String,
    age: u8,
    banned: bool,
}

let users = vec![
    User { name: "Alice".into(), age: 25, banned: false },
    User { name: "Bob".into(), age: 17, banned: false },
    User { name: "Charlie".into(), age: 30, banned: true },
    User { name: "Diana".into(), age: 22, banned: false },
];

let active_adults: Vec<String> = collect!(users)
    .reject(|u| u.banned)              // remove banned
    .filter(|u| u.age >= 18)           // keep adults
    .map(|u| u.name)                   // extract names
    .sort()                            // alphabetically
    .to_vec();

assert_eq!(active_adults, vec!["Alice", "Diana"]);
```

## Example: Pluck Pattern (Extracting Fields)

```rust
use ravel_facades::collect;

struct Product {
    id: u32,
    title: String,
    price: f64,
}

let products = vec![
    Product { id: 1, title: "Widget".into(), price: 9.99 },
    Product { id: 2, title: "Gadget".into(), price: 24.99 },
    Product { id: 3, title: "Doohickey".into(), price: 4.99 },
];

// "Pluck" the title field from every product
let titles: Vec<&str> = collect!(&products)
    .map(|p| p.title.as_str())
    .to_vec();

assert_eq!(titles, vec!["Widget", "Gadget", "Doohickey"]);

// Pluck IDs into a comma-separated string
let id_list = collect!(products.iter())
    .map(|p| p.id.to_string())
    .implode(",");

assert_eq!(id_list, "1,2,3");
```

## Conversions

`Collection<T>` converts freely to and from `Vec<T>`:

```rust
let c = Collection::from(vec![1, 2, 3]);
let v: Vec<i32> = Vec::from(c);
```

## API Reference

| Method | Description |
|--------|-------------|
| `collect!(vec)` | Create a `Collection` from a `Vec` |
| `map(f)` | Transform each element |
| `filter(f)` | Keep elements matching the predicate |
| `reject(f)` | Remove elements matching the predicate |
| `sort()` | Sort in-place (requires `Ord`) |
| `first()` | `Option<&T>` of the first element |
| `last()` | `Option<&T>` of the last element |
| `count()` | Number of elements |
| `take(n)` | Keep the first `n` elements |
| `skip(n)` | Drop the first `n` elements |
| `contains(f)` | Check if any element matches the predicate |
| `each(f)` | Run a closure for each element (consumes) |
| `implode(glue)` | Join display-able elements into a string |
| `to_vec()` | Unwrap back into `Vec<T>` |
| `is_empty()` / `is_not_empty()` | Emptiness checks |
