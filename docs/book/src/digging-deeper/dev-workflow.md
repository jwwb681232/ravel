# Ravel 框架开发命令参考

## 术语

| 术语 | 含义 |
|------|------|
| **workspace 根** | `E:\rust\ravel\` — 框架仓库根目录，包含 `Cargo.toml` 和 `crates/` |
| **项目目录** | `my-app/` — 通过 `ravel new my-app --dev` 创建的测试项目 |

## 运行 ravel-cli

开发期间不要用 `cargo install`，有三种运行方式：

```bash
# 方式 1: workspace 根 — cargo run（自动编译最新代码）
cargo run -p ravel-cli -- <command>

# 方式 2: 项目目录 — cargo run（path 依赖让 cargo 自动找到 workspace）
cargo run -p ravel-cli -- <command>

# 方式 3: 项目目录 — 已编译二进制（最快，无需重新编译 CLI）
../target/debug/ravel-cli <command>
```

方式 1 和 2 都自动编译最新的 CLI 代码。方式 3 更快但 CLI 改了代码需要先 `cargo build -p ravel-cli`。

## 初始化 dev 项目

```bash
cd E:\rust\ravel                              # 必须在 workspace 根
cargo run -p ravel-cli -- new my-app --dev
```

生成的项目使用 `path` 依赖指向本地框架 crate，修改框架代码后无需 `git push`。

同时自动将项目加入 workspace `Cargo.toml` 的 `members` 列表，因此后续在项目目录内也可以直接用 `cargo run -p ravel-cli --`。

## 开发服务器

```bash
cd my-app
../target/debug/ravel-cli serve
# 或
cargo run -p ravel-cli -- serve
```

`serve` 检测到 `.ravel-dev` 标记后，会从 workspace 根执行 `cargo run -p my-app --bin my-app`，确保 path 依赖的框架 crate 最新。

## 代码生成（Make）

```bash
cd my-app

# 两种方式等价，任选一种
../target/debug/ravel-cli make controller UserController
cargo run -p ravel-cli -- make controller UserController

../target/debug/ravel-cli make middleware AuthMiddleware
../target/debug/ravel-cli make model User
../target/debug/ravel-cli make model User -m         # 同时生成 Migration
../target/debug/ravel-cli make migration CreateUsersTable
../target/debug/ravel-cli make seeder UserSeeder
../target/debug/ravel-cli make provider RouteServiceProvider
../target/debug/ravel-cli make request LoginRequest
../target/debug/ravel-cli make job SendWelcomeEmail
```

> **注意：** `make` 和 `db` 的实际命令用空格（`make controller`、`db seed`），不是冒号（`make:controller`、`db:seed`）。READEME 中的冒号写法是 Laravel 风格文档惯例，clap 将 `Make` 和 `Db` 作为嵌套子命令解析。`migrate`、`route:list`、`key:generate` 等则通过 `#[command(name = "...")]` 显式指定了冒号名称，所以必须用冒号。

## 数据库迁移

```bash
cd my-app

# 两种方式等价
../target/debug/ravel-cli migrate
cargo run -p ravel-cli -- migrate

../target/debug/ravel-cli migrate -s 1               # 只执行 1 步
../target/debug/ravel-cli migrate:rollback            # 回滚最后一步
../target/debug/ravel-cli migrate:rollback -s 2       # 回滚 2 步
../target/debug/ravel-cli migrate:refresh             # 回滚全部 + 重新执行
../target/debug/ravel-cli migrate:fresh               # 删表重建
../target/debug/ravel-cli migrate:status              # 查看迁移状态
../target/debug/ravel-cli db seed                     # 运行 seeders
```

## 其他命令

```bash
cd my-app
../target/debug/ravel-cli route:list                 # 列出所有路由
../target/debug/ravel-cli key:generate               # 生成应用密钥
```

## 快速参考表

| 命令 | 执行目录 | target 二进制 | cargo run（项目目录） |
|------|---------|-------------|---------------------|
| `new my-app --dev` | workspace 根 | — | `cargo run -p ravel-cli -- new my-app --dev` |
| `serve` | 项目目录 | `../target/debug/ravel-cli serve` | `cargo run -p ravel-cli -- serve` |
| `make controller X` | 项目目录 | `../target/debug/ravel-cli make controller X` | `cargo run -p ravel-cli -- make controller X` |
| `migrate` | 项目目录 | `../target/debug/ravel-cli migrate` | `cargo run -p ravel-cli -- migrate` |
| `route:list` | 项目目录 | `../target/debug/ravel-cli route:list` | `cargo run -p ravel-cli -- route:list` |

## daily dev 循环

```bash
# 1. 改框架代码
vim crates/ravel-http/src/...

# 2. 进入项目目录测试
cd my-app
cargo run -p ravel-cli -- serve        # 自动编译最新 CLI + 启动

# 3. 验证 → 回到步骤 1

# 如果只改了应用代码没改 CLI，用二进制更快：
../target/debug/ravel-cli serve
```
