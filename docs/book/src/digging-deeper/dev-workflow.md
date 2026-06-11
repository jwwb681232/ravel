# Ravel 框架开发命令参考

## 术语

| 术语 | 含义 |
|------|------|
| **workspace 根** | `E:\rust\ravel\` — 框架仓库根目录，包含 `Cargo.toml` 和 `crates/` |
| **项目目录** | `my-app/` — 通过 `ravel new my-app --dev` 创建的测试项目 |

## 运行 ravel-cli

开发期间不要用 `cargo install`，直接用已编译的二进制或 `cargo run`：

```bash
# 在 workspace 根，用 cargo run（自动编译最新代码）
cargo run -p ravel-cli -- <command>

# 在项目目录，用已编译的二进制（更快）
../target/debug/ravel-cli <command>
```

## 初始化 dev 项目

```bash
cd E:\rust\ravel                              # 必须在 workspace 根
cargo run -p ravel-cli -- new my-app --dev
```

生成的项目使用 `path` 依赖指向本地框架 crate，修改框架代码后无需 `git push`。

## 开发服务器

```bash
cd my-app                                     # 必须在项目目录
../target/debug/ravel-cli serve
```

`serve` 检测到 `.ravel-dev` 标记后，会从 workspace 根执行 `cargo run -p my-app --bin my-app`，确保 path 依赖的框架 crate 最新。

## 代码生成（Make）

```bash
cd my-app
../target/debug/ravel-cli make controller UserController
../target/debug/ravel-cli make middleware AuthMiddleware
../target/debug/ravel-cli make model User
../target/debug/ravel-cli make model User -m        # 同时生成 Migration
../target/debug/ravel-cli make migration CreateUsersTable
../target/debug/ravel-cli make seeder UserSeeder
../target/debug/ravel-cli make provider RouteServiceProvider
../target/debug/ravel-cli make request LoginRequest
../target/debug/ravel-cli make job SendWelcomeEmail
```

> **注意：** 实际命令是 `make controller`（空格），不是 `make:controller`（冒号）。READEME 中的 `make:controller` 是 Laravel 风格的文档写法，clap 的实际解析是嵌套子命令。

## 数据库迁移

```bash
cd my-app
../target/debug/ravel-cli migrate                  # 执行待处理的迁移
../target/debug/ravel-cli migrate -s 1              # 只执行 1 步
../target/debug/ravel-cli migrate:rollback           # 回滚最后一步
../target/debug/ravel-cli migrate:rollback -s 2      # 回滚 2 步
../target/debug/ravel-cli migrate:refresh            # 回滚全部 + 重新执行
../target/debug/ravel-cli migrate:fresh              # 删表重建
../target/debug/ravel-cli migrate:status             # 查看迁移状态
../target/debug/ravel-cli db:seed                    # 运行 seeders
```

## 其他命令

```bash
cd my-app
../target/debug/ravel-cli route:list                # 列出所有路由
../target/debug/ravel-cli key:generate              # 生成应用密钥
```

## 快速参考表

| 命令 | 执行目录 | 等效 cargo 写法（workspace 根） |
|------|---------|-------------------------------|
| `ravel new my-app --dev` | workspace 根 | `cargo run -p ravel-cli -- new my-app --dev` |
| `ravel serve` | 项目目录 | — |
| `ravel make controller X` | 项目目录 | — |
| `ravel migrate` | 项目目录 | — |
| `ravel route:list` | 项目目录 | — |

## daily dev 循环

```bash
# 1. 改框架代码
vim crates/ravel-http/src/...

# 2. 编译 CLI（如果改了 CLI 代码）
cargo build -p ravel-cli

# 3. 进入项目目录测试
cd my-app
../target/debug/ravel-cli serve

# 4. 验证 → 回到步骤 1
```
