# Views

Ravel's view engine is powered by **Tera**, a fast and flexible template engine inspired by Jinja2 and Django templates. Views are rendered from `.html` files stored in the `resources/views/` directory.

## Creating a View Engine

Create a `View` instance pointing to your templates directory. This is typically done inside a service provider or the main function:

```rust
use ravel_http::view::View;

let view = View::new("resources/views").expect("Failed to load templates");
```

Templates are loaded once at construction time. The `View` looks for all files under the given directory.

## Rendering Templates

Call `render()` to get the rendered string, or `render_html()` to get an Axum `Html` response:

```rust
use ravel_http::view::{View, context};
use axum::response::IntoResponse;

async fn home(view: &View) -> impl IntoResponse {
    let ctx = context! {
        "title" => "Welcome",
        "year" => 2026,
    };
    view.render_html("home", &ctx).unwrap()
}
```

The template name is relative to the templates directory. The `.html` extension is appended automatically — `"home"` loads `resources/views/home.html`.

## The `context!` Macro

Use the `context!` macro to build a Tera context with key-value pairs:

```rust
use ravel_http::view::context;
use serde::Serialize;

#[derive(Serialize)]
struct User {
    name: String,
    email: String,
}

let user = User {
    name: "Alice".into(),
    email: "alice@example.com".into(),
};

let ctx = context! {
    "page_title" => "User Profile",
    "user" => &user,
    "is_admin" => true,
    "items" => &vec!["a", "b", "c"],
};
```

## Template Syntax

Tera uses `{{ variables }}` for output and `{% tags %}` for control flow.

### Variables

```html
<!-- resources/views/home.html -->
<h1>{{ title }}</h1>
<p>Welcome, {{ user.name }}!</p>
<p>Email: {{ user.email }}</p>
```

### Filters

```html
<p>{{ name | upper }}</p>
<p>{{ description | truncate(length=100) }}</p>
<p>{{ created_at | date(format="%Y-%m-%d") }}</p>
```

### Loops

```html
<ul>
{% for post in posts %}
    <li>
        <h2><a href="/posts/{{ post.slug }}">{{ post.title }}</a></h2>
        <p>{{ post.excerpt }}</p>
    </li>
{% else %}
    <li>No posts found.</li>
{% endfor %}
</ul>
```

### Conditionals

```html
{% if user.is_admin %}
    <a href="/admin/dashboard">Admin Panel</a>
{% elif user.is_logged_in %}
    <a href="/profile">My Profile</a>
{% else %}
    <a href="/login">Log In</a>
{% endif %}
```

### Template Inheritance

```html
<!-- resources/views/layouts/app.html -->
<!DOCTYPE html>
<html>
<head>
    <title>{% block title %}My App{% endblock %}</title>
</head>
<body>
    <nav><!-- ... --></nav>
    <main>
        {% block content %}{% endblock %}
    </main>
</body>
</html>
```

```html
<!-- resources/views/posts/index.html -->
{% extends "layouts/app.html" %}

{% block title %}Blog Posts{% endblock %}

{% block content %}
    <h1>Blog Posts</h1>
    {% for post in posts %}
        <article>
            <h2>{{ post.title }}</h2>
            <p>{{ post.body }}</p>
        </article>
    {% endfor %}
{% endblock %}
```

## Using Views in Handlers

Pass the `View` instance to your handlers. Here is a complete example:

```rust
use ravel_http::view::{View, context};
use ravel_facades::Route;
use axum::response::IntoResponse;
use axum::extract::State;

async fn blog_index(State(view): State<View>) -> impl IntoResponse {
    let posts = vec![
        Post { title: "First Post".into(), body: "Hello world!".into() },
        Post { title: "Second Post".into(), body: "Rust is great!".into() },
    ];

    let ctx = context! {
        "title" => "My Blog",
        "posts" => &posts,
    };

    view.render_html("posts/index", &ctx).unwrap()
}

struct Post {
    title: String,
    body: String,
}

// Create View and route
let view = View::new("resources/views").unwrap();
Route::get("/", blog_index);
let router = Route::build().with_state(view);
```

## View Composers

View composers are a planned feature that will allow you to bind data to a template automatically whenever it is rendered. This is useful for injecting shared data (like navigation menus or user information) without repeating it in every handler.

In the current release, simply build your context before rendering:

```rust
// Instead of a view composer, create a helper function
fn shared_context() -> Context {
    context! {
        "app_name" => "MyApp",
        "current_year" => 2026,
        "navigation" => &load_navigation(),
    }
}

let mut ctx = shared_context();
ctx.insert("page_title", &"Dashboard");
ctx.insert("user", &current_user);
view.render_html("dashboard", &ctx).unwrap()
```

## Complete Example: Blog Post List

```rust
use ravel_http::view::{View, context};
use ravel_http::server;
use axum::response::IntoResponse;
use axum::extract::State;

#[derive(serde::Serialize)]
struct BlogPost {
    title: String,
    excerpt: String,
    published_at: String,
}

async fn list_posts(State(view): State<View>) -> impl IntoResponse {
    let posts = vec![
        BlogPost {
            title: "Getting Started with Ravel".into(),
            excerpt: "Learn how to build web applications in Rust...".into(),
            published_at: "2026-06-01".into(),
        },
        BlogPost {
            title: "Middleware Deep Dive".into(),
            excerpt: "Understanding middleware ordering and customization...".into(),
            published_at: "2026-06-05".into(),
        },
    ];

    let ctx = context! {
        "page_title" => "Blog",
        "posts" => &posts,
    };

    view.render_html("blog/index", &ctx).unwrap()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let view = View::new("resources/views")?;

    Route::get("/blog", list_posts);
    let router = Route::build().with_state(view);

    server::serve(router, "127.0.0.1:3000").await
}
```

With the corresponding template at `resources/views/blog/index.html`:

```html
{% extends "layouts/app.html" %}

{% block title %}{{ page_title }}{% endblock %}

{% block content %}
    <h1>{{ page_title }}</h1>

    {% for post in posts %}
        <article class="post">
            <h2><a href="#">{{ post.title }}</a></h2>
            <p class="excerpt">{{ post.excerpt }}</p>
            <small>Published {{ post.published_at }}</small>
        </article>
    {% else %}
        <p>No posts yet. Check back soon!</p>
    {% endfor %}
{% endblock %}
```
