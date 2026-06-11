# Rich Scaffold: 丰富项目模板

**Date:** 2026-06-11
**Status:** approved

## 1. Problem

`ravel new` 生成的项目过于空洞——只有空目录和一个最简单的 `Route::new().get("/")` 路由。用户无法从脚手架看到框架能力的全貌，必须逐一查找源码才知道怎么用 Controller、Model、Auth 等功能。

## 2. Design

### 2.1 覆盖目标（Level B — 参考级）

包含以下功能，不含 Views 和 Tests：

| 功能 | 载体 |
|------|------|
| Application boot 完整流程 | `src/main.rs` |
| ServiceProvider | `AppServiceProvider.rs`、`RouteServiceProvider.rs` |
| Config facade | `UserController.rs` |
| Route + route group + middleware | `routes/web.rs` |
| Controller trait | `UserController.rs`、`PostController.rs` |
| Auth + Session | `UserController.rs`（login/logout/check/id） |
| RavelError + abort + ? 传播 | `PostController.rs` |
| response().json() + redirect() | `UserController.rs` |
| #[derive(Model)] + CRUD + Relations | `User.rs`、`Post.rs`（HasMany + BelongsTo） |
| FormRequest + validation rules | `CreatePostRequest.rs` |
| Queue + #[derive(Job)] | `SendWelcomeEmail.rs` |
| Cache | `src/main.rs`（with_cache） |
| Migration | `0001_create_users_table.rs`、`0002_create_posts_table.rs` |
| Seeder | `UserSeeder.rs` |

### 2.2 文件结构

```
{{name}}/
├── Cargo.toml
├── .env / .env.example
├── config/
│   ├── app.toml
│   └── database.toml              ← NEW
├── src/
│   ├── main.rs                    ← Application::new().boot()
│   └── bin/{migrate,seed}.rs
├── routes/
│   └── web.rs                     ← NEW（路由注册函数）
├── app/
│   ├── Http/
│   │   ├── Controllers/
│   │   │   ├── UserController.rs  ← NEW
│   │   │   └── PostController.rs  ← NEW
│   │   ├── Middleware/            ← 空目录
│   │   └── Requests/
│   │       └── CreatePostRequest.rs ← NEW
│   ├── Models/
│   │   ├── User.rs                ← NEW
│   │   └── Post.rs                ← NEW
│   ├── Jobs/
│   │   └── SendWelcomeEmail.rs    ← NEW
│   ├── Providers/
│   │   ├── AppServiceProvider.rs  ← NEW
│   │   └── RouteServiceProvider.rs ← NEW
│   └── Services/                  ← 空目录
├── database/
│   ├── migrations/
│   │   ├── mod.rs
│   │   ├── 0001_create_users_table.rs  ← NEW
│   │   └── 0002_create_posts_table.rs  ← NEW
│   └── seeders/
│       └── UserSeeder.rs          ← NEW
├── storage/                       ← 空目录
└── bootstrap/                     ← 空目录
```

### 2.3 各文件职责与关键代码

#### src/main.rs — 启动入口

```rust
use ravel_core::app::Application;
use ravel_facades::Route;
use ravel_http::server;

mod routes;
mod app;
use app::Providers::{AppServiceProvider, RouteServiceProvider};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = Application::new()
        .load_env(".")?
        .load_config("config")?
        .with_cache()
        .with_app_key("base64:...")?
        .register_provider(AppServiceProvider)
        .register_provider(RouteServiceProvider)
        .boot()?;

    let router = Route::build();
    let host = "127.0.0.1:3000";
    println!("{{name}} running at http://{host}");
    server::serve(router, host).await?;
    Ok(())
}
```

#### routes/web.rs — 路由定义

注册一个全局日志中间件，然后定义首页、用户路由（含 login/logout）、API 路由组。

#### app/Providers/RouteServiceProvider.rs

在 `boot()` 中调用 `crate::routes::web::register()`。

#### app/Providers/AppServiceProvider.rs

在 `register()` 中调用 `Queue::memory()`。

#### app/Http/Controllers/UserController.rs

4 个方法：
- `index()` — `Config::get_or()` + `Log::info()` + `response().json()`
- `show(id, session)` — `Auth::guest()` / `Auth::id()` + `redirect()` + `RavelError` 返回
- `login(session)` — `Auth::login()` + `Session::flash()`
- `logout(session)` — `Auth::logout()` + `redirect()`

#### app/Http/Controllers/PostController.rs

- `store(Validated(req), db)` — `Post::create()` + `Queue::dispatch(SendWelcomeEmail)` + `abort()` / `to_public_json()`

#### app/Models/User.rs

```rust
#[derive(Model)]
#[model(table = "users", timestamps)]
pub struct User {
    #[model(id)]             pub id: i32,
    #[model(string, 255)]    pub name: String,
    #[model(string, 254, unique)] pub email: String,
    #[model(hidden)]         pub password: String,
    pub posts: HasMany<Post>,
}
```

#### app/Models/Post.rs

```rust
#[derive(Model)]
#[model(table = "posts", timestamps)]
pub struct Post {
    #[model(id)]             pub id: i32,
    #[model(string, 255)]    pub title: String,
    #[model(text)]           pub content: String,
    #[model(belongs_to, from = "user_id", to = "id")]
    pub user_id: i32,
    pub user: BelongsTo<User>,
}
```

#### app/Http/Requests/CreatePostRequest.rs

```rust
impl FormRequest for CreatePostRequest {
    fn rules() -> Vec<FieldRule> {
        vec![
            FieldRule::new("title", vec![Rule::Required, Rule::Min(3), Rule::Max(200)]),
            FieldRule::new("content", vec![Rule::Required]),
            FieldRule::new("user_id", vec![Rule::Required,
                Rule::Exists { table: "users", column: "id", ignore_id: None }]),
        ]
    }
}
```

#### app/Jobs/SendWelcomeEmail.rs

```rust
#[derive(Serialize, Deserialize, Job)]
#[job(name = "send_welcome_email")]
pub struct SendWelcomeEmail { pub user_id: i32 }

impl Job for SendWelcomeEmail {
    async fn handle(&self) -> Result<()> {
        Log::info!("Welcome email sent to user {}", self.user_id);
        Ok(())
    }
}
```

#### database/migrations/ — 两张表

`users` 表（id, name, email, password, created_at, updated_at）+ `posts` 表（id, user_id FK, title, content, created_at, updated_at）。

#### database/seeders/UserSeeder.rs

使用 `sea_orm::DatabaseConnection` 插入示例用户数据。

### 2.4 模板实现

在 `ravel-cli/src/generator.rs` 中新增以下 Tera 模板：

| 模板名 | 对应文件 |
|--------|---------|
| `user_controller` | app/Http/Controllers/UserController.rs |
| `post_controller` | app/Http/Controllers/PostController.rs |
| `user_model` | app/Models/User.rs |
| `post_model` | app/Models/Post.rs |
| `create_post_request` | app/Http/Requests/CreatePostRequest.rs |
| `send_welcome_job` | app/Jobs/SendWelcomeEmail.rs |
| `app_service_provider` | app/Providers/AppServiceProvider.rs |
| `route_service_provider` | app/Providers/RouteServiceProvider.rs |
| `routes_web` | routes/web.rs |
| `database_toml` | config/database.toml |
| `main_rs` | src/main.rs（重写） |
| `migration_users` | database/migrations/0001_create_users_table.rs |
| `migration_posts` | database/migrations/0002_create_posts_table.rs |
| `user_seeder` | database/seeders/UserSeeder.rs |

修改 `main_rs`、`app_toml`、`migrator` 模板以适配新结构。

### 2.5 Generator 改动

`scaffold_project()` 方法中：
- 新增 `mod app { pub mod Http { pub mod Controllers; pub mod Requests; } pub mod Models; pub mod Jobs; pub mod Providers; }` 目录 + mod.rs
- 调用 14 个新模板渲染并写入对应文件
- `main.rs` 用新模板替换旧的简单模板

### 2.6 新增依赖

生成的 Cargo.toml 需要额外引入项目直接使用的 crate：
- `ravel-support`（Job trait、Queue facade）
- `ravel-macros`（`#[derive(Job)]`）

## 3. Non-Goals

- 不包含 Views（Tera）示例
- 不包含 Test（TestClient）示例
- 不使用 Redis 驱动（Queue 用内存）
- 不包含 CSRF、Rate Limiting、File Upload、JWT 示例
- 不修改 `make:*` 命令的行为

## 4. Files Changed

| # | 文件 | 改动 |
|---|------|------|
| 1 | `crates/ravel-cli/src/generator.rs` | 新增 14 个模板常量 + 修改 `scaffold_project` |
