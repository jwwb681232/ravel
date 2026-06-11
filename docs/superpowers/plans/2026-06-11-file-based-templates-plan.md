# File-Based Templates for `ravel new` — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract 29 inline string templates from `generator.rs` into `templates/` directory files, loaded at compile time via `include_dir!`.

**Architecture:** Replace `tera.add_raw_template()` + `tera.render()` (pre-registered templates) with `include_dir!` (compile-time embedded files) + `tera.render_str()` (on-the-fly rendering). All scaffold methods keep the same signatures; only the internal render path changes.

**Tech Stack:** `include_dir = "0.7"`, existing `tera` with `render_str` API

**Template name ↔ file name mapping:**

| Tera name | File name |
|-----------|-----------|
| `controller` | `controller.rs` |
| `middleware` | `middleware.rs` |
| `migration` | `migration.rs` |
| `seeder` | `seeder.rs` |
| `provider` | `provider.rs` |
| `request` | `request.rs` |
| `model` | `model.rs` |
| `job` | `job.rs` |
| `cargo_toml` | `cargo-release.toml` |
| `cargo_toml_dev` | `cargo-dev.toml` |
| `main_rs` | `main.rs` |
| `app_toml` | `app.toml` |
| `env` | `env` |
| `database_toml` | `database.toml` |
| `routes_web` | `routes-web.rs` |
| `user_model` | `user-model.rs` |
| `post_model` | `post-model.rs` |
| `user_controller` | `user-controller.rs` |
| `post_controller` | `post-controller.rs` |
| `create_post_request` | `create-post-request.rs` |
| `send_welcome_job` | `send-welcome-job.rs` |
| `app_service_provider` | `app-service-provider.rs` |
| `route_service_provider` | `route-service-provider.rs` |
| `migration_users` | `migration-users.rs` |
| `migration_posts` | `migration-posts.rs` |
| `user_seeder` | `user-seeder.rs` |
| `migrator` | `migrator.rs` |
| `migrate_bin` | `migrate-bin.rs` |
| `seed_bin` | `seed-bin.rs` |

**Key design decision:** The `render()` method signature stays `render(&self, file_name: &str, name: &str)`, but callers now pass the exact file name (e.g. `"controller.rs"`) instead of the Tera template name (`"controller"`). The method looks up the file from the embedded `Dir`, extracts its UTF-8 content, and passes it to `tera.render_str()`.

---

### Task 1: Add dependency and create template directory

**Files:**
- Modify: `crates/ravel-cli/Cargo.toml`
- Create: `crates/ravel-cli/templates/` (29 files)

- [ ] **Step 1: Add `include_dir` to Cargo.toml**

```toml
# In [dependencies], add:
include_dir = "0.7"
```

- [ ] **Step 2: Create the templates directory**

```bash
mkdir crates/ravel-cli/templates
```

- [ ] **Step 3: Create each template file**

For each template constant in `generator.rs`, create a file in `templates/`. The content is the raw template text between `r#"` and `"#;` — no escaping needed, no trailing `"#;`. Remove leading tab indentation (the templates inside `generator.rs` are indented with one tab, but the files should be flush-left).

File list and content source:
- `templates/controller.rs` ← `CONTROLLER_TEMPLATE`
- `templates/middleware.rs` ← `MIDDLEWARE_TEMPLATE`
- `templates/migration.rs` ← `MIGRATION_TEMPLATE`
- `templates/seeder.rs` ← `SEEDER_TEMPLATE`
- `templates/provider.rs` ← `PROVIDER_TEMPLATE`
- `templates/request.rs` ← `REQUEST_TEMPLATE`
- `templates/model.rs` ← `MODEL_TEMPLATE`
- `templates/job.rs` ← `JOB_TEMPLATE`
- `templates/cargo-release.toml` ← `CARGO_TOML_TEMPLATE`
- `templates/cargo-dev.toml` ← `CARGO_TOML_DEV_TEMPLATE`
- `templates/main.rs` ← `MAIN_RS_TEMPLATE`
- `templates/app.toml` ← `APP_TOML_TEMPLATE`
- `templates/env` ← `ENV_TEMPLATE`
- `templates/database.toml` ← `DATABASE_TOML_TEMPLATE`
- `templates/routes-web.rs` ← `ROUTES_WEB_TEMPLATE`
- `templates/user-model.rs` ← `USER_MODEL_TEMPLATE`
- `templates/post-model.rs` ← `POST_MODEL_TEMPLATE`
- `templates/user-controller.rs` ← `USER_CONTROLLER_TEMPLATE`
- `templates/post-controller.rs` ← `POST_CONTROLLER_TEMPLATE`
- `templates/create-post-request.rs` ← `CREATE_POST_REQUEST_TEMPLATE`
- `templates/send-welcome-job.rs` ← `SEND_WELCOME_JOB_TEMPLATE`
- `templates/app-service-provider.rs` ← `APP_SERVICE_PROVIDER_TEMPLATE`
- `templates/route-service-provider.rs` ← `ROUTE_SERVICE_PROVIDER_TEMPLATE`
- `templates/migration-users.rs` ← `MIGRATION_USERS_TEMPLATE`
- `templates/migration-posts.rs` ← `MIGRATION_POSTS_TEMPLATE`
- `templates/user-seeder.rs` ← `USER_SEEDER_TEMPLATE`
- `templates/migrator.rs` ← `MIGRATOR_TEMPLATE`
- `templates/migrate-bin.rs` ← `MIGRATE_BIN_TEMPLATE`
- `templates/seed-bin.rs` ← `SEED_BIN_TEMPLATE`

**Example — `templates/controller.rs`:**

```rust
use ravel_http::controller::Controller;
use ravel_core::container::Container;

pub struct {{name}};

impl Controller for {{name}} {
    fn boot(_container: &Container) -> Self {
        Self
    }
}

// ── Handlers ──────────────────────────────────────────────────────
//
// impl {{name}} {
//     pub async fn index() -> impl axum::response::IntoResponse {
//         "{{name}} index"
//     }
// }
```

**Example — `templates/env`:**

```
APP_NAME={{name}}
APP_ENV=local
APP_DEBUG=true
APP_URL=http://localhost:3000
APP_KEY=base64:{{app_key}}
```

- [ ] **Step 4: Verify template files compile as valid UTF-8**

```bash
# Quick sanity — all files should be non-empty and valid UTF-8
for f in crates/ravel-cli/templates/*; do
  echo "$f: $(wc -c < "$f") bytes"
done
```

- [ ] **Step 5: Commit**

```bash
git add crates/ravel-cli/Cargo.toml crates/ravel-cli/templates/
git commit -m "feat: add include_dir dep and 29 template files"
```

---

### Task 2: Rewrite Generator struct and render method

**Files:**
- Modify: `crates/ravel-cli/src/generator.rs:1-140`

- [ ] **Step 1: Add imports and static Dir**

Replace the imports at the top of `generator.rs`. Add `include_dir` import:

```rust
use anyhow::{Context, Result, bail};
use base64::Engine;
use chrono::Utc;
use include_dir::{include_dir, Dir};
use rand::RngCore;
use std::fs;
use std::path::PathBuf;
use tera::{Context as TeraContext, Tera};
```

Add the static embedded templates, right after the imports (before `pub struct Generator`):

```rust
static TEMPLATES: Dir = include_dir!("$CARGO_MANIFEST_DIR/templates");
```

- [ ] **Step 2: Simplify `Generator::new()` — remove all `add_raw_template` calls**

Replace the entire `new()` method body:

```rust
impl Generator {
    /// Create a generator targeting `root` as the project root.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            tera: Tera::default(),
        }
    }
```

Remove lines 37-81 (all `tera.add_raw_template(...)` calls), keep only the struct init.

- [ ] **Step 3: Rewrite `render()` method to use embedded Dir + `render_str`**

Replace the current `render()` method:

```rust
    /// Render a template file by name with name-derived variables.
    ///
    /// `file_name` is the path relative to `templates/` (e.g. `"controller.rs"`).
    pub fn render(&self, file_name: &str, name: &str) -> Result<String> {
        let ctx = Self::make_context(name);
        let file = TEMPLATES
            .get_file(file_name)
            .with_context(|| format!("Template not found: '{file_name}'"))?;
        let src = file
            .contents_utf8()
            .with_context(|| format!("Template '{file_name}' is not valid UTF-8"))?;
        self.tera
            .render_str(src, &ctx)
            .with_context(|| format!("Failed to render template '{file_name}'"))
    }
```

- [ ] **Step 4: Add a helper to render a template with a custom context**

Add this method for cases where extra context variables are needed (like `app_key` for the env template):

```rust
    /// Render a template with an already-built Tera context.
    pub fn render_with_context(
        &self,
        file_name: &str,
        ctx: &TeraContext,
    ) -> Result<String> {
        let file = TEMPLATES
            .get_file(file_name)
            .with_context(|| format!("Template not found: '{file_name}'"))?;
        let src = file
            .contents_utf8()
            .with_context(|| format!("Template '{file_name}' is not valid UTF-8"))?;
        self.tera
            .render_str(src, ctx)
            .with_context(|| format!("Failed to render template '{file_name}'"))
    }
```

- [ ] **Step 5: Verify compilation fails (expected — callers still use old names)**

```bash
cd E:/rust/ravel && cargo check -p ravel-cli 2>&1 | head -3
```

Expected: fails because callers pass `"controller"` (no extension) and `tera.add_raw_template` calls are gone.

- [ ] **Step 6: Commit**

```bash
git add crates/ravel-cli/src/generator.rs
git commit -m "refactor: replace add_raw_template with include_dir! + render_str"
```

---

### Task 3: Update all `render()` call sites in scaffold methods

**Files:**
- Modify: `crates/ravel-cli/src/generator.rs` (scaffold_controller through scaffold_job, lines 144-201)

- [ ] **Step 1: Update all `self.render()` calls to use file names**

Each `self.render("template_name", name)?` → `self.render("file-name.ext", name)?`:

| Old call | New call |
|----------|----------|
| `self.render("controller", name)?` | `self.render("controller.rs", name)?` |
| `self.render("middleware", name)?` | `self.render("middleware.rs", name)?` |
| `self.render("migration", name)?` | `self.render("migration.rs", name)?` |
| `self.render("seeder", name)?` | `self.render("seeder.rs", name)?` |
| `self.render("provider", name)?` | `self.render("provider.rs", name)?` |
| `self.render("request", name)?` | `self.render("request.rs", name)?` |
| `self.render("model", name)?` | `self.render("model.rs", name)?` |
| `self.render("job", name)?` | `self.render("job.rs", name)?` |

- [ ] **Step 2: Verify compilation**

```bash
cd E:/rust/ravel && cargo check -p ravel-cli 2>&1
```

- [ ] **Step 3: Commit**

```bash
git add crates/ravel-cli/src/generator.rs
git commit -m "refactor: update scaffold method render calls to use file names"
```

---

### Task 4: Update `scaffold_project()` — 20+ template references

**Files:**
- Modify: `crates/ravel-cli/src/generator.rs` (scaffold_project method, lines 209-340)

- [ ] **Step 1: Update `self.render()` calls in scaffold_project**

| Old call | New call |
|----------|----------|
| `self.render(cargo_template, project_name)?` where cargo_template is `"cargo_toml"` | `self.render("cargo-release.toml", project_name)?` |
| `self.render(cargo_template, project_name)?` where cargo_template is `"cargo_toml_dev"` | `self.render("cargo-dev.toml", project_name)?` |
| `self.render("main_rs", project_name)?` | `self.render("main.rs", project_name)?` |
| `self.render("app_toml", project_name)?` | `self.render("app.toml", project_name)?` |
| `self.render("routes_web", project_name)?` | `self.render("routes-web.rs", project_name)?` |
| `self.render("user_model", project_name)?` | `self.render("user-model.rs", project_name)?` |
| `self.render("post_model", project_name)?` | `self.render("post-model.rs", project_name)?` |
| `self.render("user_controller", project_name)?` | `self.render("user-controller.rs", project_name)?` |
| `self.render("post_controller", project_name)?` | `self.render("post-controller.rs", project_name)?` |

- [ ] **Step 2: Replace direct template usage with `render_with_context` / `render_str`**

**env template:** Replace
```rust
let env_content = self
    .tera
    .render("env", &env_ctx)
    .context("Failed to render env template")?;
```
With:
```rust
let env_content = self.render_with_context("env", &env_ctx)?;
```

**Static templates (no variables):** Replace raw constant usage with `render_with_context` + empty context, OR just look up and get raw content. For truly static templates, use:

```rust
fn raw_template(file_name: &str) -> Result<&'static str> {
    let file = TEMPLATES
        .get_file(file_name)
        .with_context(|| format!("Template not found: '{file_name}'"))?;
    file.contents_utf8()
        .with_context(|| format!("Template '{file_name}' is not valid UTF-8"))
}
```

Then replace each direct template constant with `raw_template("...")?`:

| Old code | New code |
|----------|----------|
| `self.overwrite_file("config/database.toml", DATABASE_TOML_TEMPLATE)?;` | `self.overwrite_file("config/database.toml", raw_template("database.toml")?)?;` |
| `self.create_file("app/Http/Requests/create_post_request.rs", CREATE_POST_REQUEST_TEMPLATE)?;` | `self.create_file("app/Http/Requests/create_post_request.rs", raw_template("create-post-request.rs")?)?;` |
| `self.create_file("app/Jobs/send_welcome_email.rs", SEND_WELCOME_JOB_TEMPLATE)?;` | `self.create_file("app/Jobs/send_welcome_email.rs", raw_template("send-welcome-job.rs")?)?;` |
| `self.create_file("app/Providers/app_service_provider.rs", APP_SERVICE_PROVIDER_TEMPLATE)?;` | `self.create_file("app/Providers/app_service_provider.rs", raw_template("app-service-provider.rs")?)?;` |
| `self.create_file("app/Providers/route_service_provider.rs", ROUTE_SERVICE_PROVIDER_TEMPLATE)?;` | `self.create_file("app/Providers/route_service_provider.rs", raw_template("route-service-provider.rs")?)?;` |
| `self.create_file("database/migrations/m0001_create_users_table.rs", MIGRATION_USERS_TEMPLATE)?;` | `self.create_file("database/migrations/m0001_create_users_table.rs", raw_template("migration-users.rs")?)?;` |
| `self.create_file("database/migrations/m0002_create_posts_table.rs", MIGRATION_POSTS_TEMPLATE)?;` | `self.create_file("database/migrations/m0002_create_posts_table.rs", raw_template("migration-posts.rs")?)?;` |
| `self.create_file("database/seeders/UserSeeder.rs", USER_SEEDER_TEMPLATE)?;` | `self.create_file("database/seeders/UserSeeder.rs", raw_template("user-seeder.rs")?)?;` |
| `self.overwrite_file("database/migrations/mod.rs", MIGRATOR_TEMPLATE)?;` | `self.overwrite_file("database/migrations/mod.rs", raw_template("migrator.rs")?)?;` |
| `self.overwrite_file("src/bin/migrate.rs", MIGRATE_BIN_TEMPLATE)?;` | `self.overwrite_file("src/bin/migrate.rs", raw_template("migrate-bin.rs")?)?;` |
| `self.overwrite_file("src/bin/seed.rs", SEED_BIN_TEMPLATE)?;` | `self.overwrite_file("src/bin/seed.rs", raw_template("seed-bin.rs")?)?;` |

- [ ] **Step 3: Simplify cargo_template selection**

Replace:
```rust
let cargo_template = if dev {
    "cargo_toml_dev"
} else {
    "cargo_toml"
};
self.overwrite_file(
    "Cargo.toml",
    &self.render(cargo_template, project_name)?,
)?;
```
With:
```rust
let cargo_file = if dev {
    "cargo-dev.toml"
} else {
    "cargo-release.toml"
};
self.overwrite_file(
    "Cargo.toml",
    &self.render(cargo_file, project_name)?,
)?;
```

- [ ] **Step 4: Verify compilation**

```bash
cd E:/rust/ravel && cargo check -p ravel-cli 2>&1
```

- [ ] **Step 5: Commit**

```bash
git add crates/ravel-cli/src/generator.rs
git commit -m "refactor: update scaffold_project to use file-based templates"
```

---

### Task 5: Remove all 29 `const ... TEMPLATE` constants

**Files:**
- Modify: `crates/ravel-cli/src/generator.rs` (the entire `// ── Templates ──` section, ~600 lines)

- [ ] **Step 1: Delete the Templates section**

Delete everything from `// ── Templates ──` (line ~373) to the end of the last template constant (before `// ── Tests ──`). That's all 29 constants:

```
CONTROLLER_TEMPLATE
MIDDLEWARE_TEMPLATE
MIGRATION_TEMPLATE
SEEDER_TEMPLATE
PROVIDER_TEMPLATE
JOB_TEMPLATE
REQUEST_TEMPLATE
MODEL_TEMPLATE
CARGO_TOML_TEMPLATE
CARGO_TOML_DEV_TEMPLATE
MAIN_RS_TEMPLATE
APP_TOML_TEMPLATE
ENV_TEMPLATE
DATABASE_TOML_TEMPLATE
ROUTES_WEB_TEMPLATE
USER_MODEL_TEMPLATE
POST_MODEL_TEMPLATE
USER_CONTROLLER_TEMPLATE
POST_CONTROLLER_TEMPLATE
CREATE_POST_REQUEST_TEMPLATE
SEND_WELCOME_JOB_TEMPLATE
APP_SERVICE_PROVIDER_TEMPLATE
ROUTE_SERVICE_PROVIDER_TEMPLATE
MIGRATION_USERS_TEMPLATE
MIGRATION_POSTS_TEMPLATE
USER_SEEDER_TEMPLATE
MIGRATOR_TEMPLATE
MIGRATE_BIN_TEMPLATE
SEED_BIN_TEMPLATE
```

- [ ] **Step 2: Verify compilation**

```bash
cd E:/rust/ravel && cargo check -p ravel-cli 2>&1
```

Expected: clean compile (no more references to old constants).

- [ ] **Step 3: Commit**

```bash
git add crates/ravel-cli/src/generator.rs
git commit -m "refactor: remove all 29 inline template constants"
```

---

### Task 6: Run tests and verify

**Files:**
- (none modified)

- [ ] **Step 1: Run ravel-cli tests**

```bash
cd E:/rust/ravel && cargo test -p ravel-cli 2>&1
```

Expected: 17 passed, 0 failed.

- [ ] **Step 2: Run full workspace tests**

```bash
cd E:/rust/ravel && cargo test --workspace 2>&1 | grep -E "test result:|FAILED"
```

Expected: all `0 failed`.

- [ ] **Step 3: Verify my-app still compiles and runs**

```bash
cd E:/rust/ravel && cargo build -p my-app --bin my-app 2>&1 | grep -E "Finished|error"
cd E:/rust/ravel && timeout 5 cargo run -p my-app --bin my-app 2>&1 | grep "booted"
```

Expected: `Application booted successfully`.

- [ ] **Step 4: Spot-check generated output (optional)**

Create a temp project and verify it looks correct:

```bash
cd /tmp && rm -rf test-scaffold
cargo run -p ravel-cli --bin ravel -- new test-scaffold
ls -R test-scaffold/app test-scaffold/routes test-scaffold/src
```

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "chore: final verification — all tests pass"
```
