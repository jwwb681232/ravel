# ravel new --dev Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `--dev` flag to `ravel new` that creates a project with `path` dependencies to the local framework source, and make `ravel serve` workspace-aware.

**Architecture:** Five files changed. `ravel-cli main.rs` adds the `--dev` flag; `new.rs` validates we're inside the framework repo; `ravel-generator` gains a dual-template system for Cargo.toml (git vs path deps) and writes a `.ravel-dev` marker; `serve.rs` detects `.ravel-dev` and runs from workspace root; workspace `Cargo.toml` drops the `exclude` so path-dep projects coexist.

**Tech Stack:** Rust 2024 edition, clap v4, tera v1, anyhow

---

### Task 1: Remove `exclude = ["MyApp"]` from workspace Cargo.toml

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Remove the exclude line**

Open `Cargo.toml` (workspace root) and remove line 18 which is `exclude = ["MyApp"]`. The workspace `members` already lists all crates explicitly, and the exclude only existed because MyApp used git deps which can't coexist in the same workspace. Once we switch to path deps (Task 3), MyApp/my-app can coexist.

After the edit, the file should be:

```toml
# Cargo.toml

[workspace]
members = [
    "crates/ravel-cli",
    "crates/ravel-core",
    "crates/ravel-db-seaorm",
    "crates/ravel-eloquent",
    "crates/ravel-eloquent-macros",
    "crates/ravel-error",
    "crates/ravel-http",
    "crates/ravel-generator",
    "crates/ravel-macros",
    "crates/ravel-support",
    "crates/ravel-facades",
    "crates/ravel-test",
]

resolver = "2"
```

- [ ] **Step 2: Verify workspace still compiles**

```bash
cargo check --workspace
```

Expected: all crates check successfully. MyApp will NOT be picked up because it's not in `members` — only `crates/*` are.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore(workspace): remove exclude = ['MyApp'] for dev mode support"
```

---

### Task 2: Add `--dev` flag to `ravel new` command

**Files:**
- Modify: `crates/ravel-cli/src/main.rs`
- Modify: `crates/ravel-cli/src/commands/new.rs`

- [ ] **Step 1: Add `--dev` flag to New subcommand in main.rs**

In `crates/ravel-cli/src/main.rs`, find the `Commands::New` variant (around line 20). Add a `#[arg(long)] dev: bool` field:

```rust
#[derive(Subcommand)]
enum Commands {
    /// Create a new Ravel project
    New {
        /// Project name (CamelCase)
        name: String,

        /// Create a dev-mode project with path dependencies to the local framework
        #[arg(long)]
        dev: bool,
    },
    // ... rest unchanged
}
```

- [ ] **Step 2: Pass `dev` flag through to new::handle**

In the match arm for `Commands::New` (around line 109), pass the `dev` flag:

```rust
Commands::New { name, dev } => {
    commands::new::handle(&name, dev)?;
}
```

- [ ] **Step 3: Update new.rs to validate framework root and pass dev to generator**

Replace `crates/ravel-cli/src/commands/new.rs`:

```rust
use anyhow::{Result, bail};
use ravel_generator::Generator;
use std::path::Path;

/// Check whether the current directory is the Ravel framework root.
/// The framework root contains `crates/ravel-cli/Cargo.toml`.
fn is_framework_root() -> bool {
    Path::new("crates/ravel-cli/Cargo.toml").exists()
}

pub fn handle(project_name: &str, dev: bool) -> Result<()> {
    if dev && !is_framework_root() {
        bail!("--dev can only be used inside the Ravel framework repository");
    }

    let root = Path::new(project_name);

    if root.exists() {
        bail!("Directory '{}' already exists", project_name);
    }

    let g = Generator::new(root);
    g.scaffold_project(project_name, dev)?;

    println!("✅ Project created: {}", project_name);
    println!();
    if dev {
        println!("   Dev mode active — using path dependencies to local framework");
        println!("   Run with: cargo run -p ravel-cli -- serve");
    } else {
        println!("  cd {}", project_name);
        println!("  ravel serve");
    }

    Ok(())
}
```

- [ ] **Step 4: Verify CLI still compiles**

```bash
cargo check -p ravel-cli
```

Expected: pass. (It won't fully compile yet because `Generator::scaffold_project` signature hasn't changed — that's Task 3.)

- [ ] **Step 5: Commit**

```bash
git add crates/ravel-cli/src/main.rs crates/ravel-cli/src/commands/new.rs
git commit -m "feat(cli): add --dev flag to ravel new command"
```

---

### Task 3: Update ravel-generator for dev mode

**Files:**
- Modify: `crates/ravel-generator/src/lib.rs`

This is the largest change. The generator needs:
1. A second Cargo.toml template with `path` dependencies
2. `scaffold_project` to accept `dev: bool` and choose the right template
3. Write `.ravel-dev` marker file in dev mode

- [ ] **Step 1: Add the dev Cargo.toml template**

Add a new constant `CARGO_TOML_DEV_TEMPLATE` after `CARGO_TOML_TEMPLATE` (around line 503):

```rust
const CARGO_TOML_DEV_TEMPLATE: &str = r#"[package]
name = "{{snake}}"
version = "0.1.0"
edition = "2024"

[dependencies]
ravel-core     = { path = "../crates/ravel-core" }
ravel-http     = { path = "../crates/ravel-http" }
ravel-eloquent = { path = "../crates/ravel-eloquent" }
ravel-facades  = { path = "../crates/ravel-facades" }
ravel-db-seaorm = { path = "../crates/ravel-db-seaorm" }
sea-orm             = { version = "2.0.0-rc.40", features = ["sqlx-sqlite", "runtime-tokio-rustls"] }
sea-orm-migration   = { version = "2.0.0-rc.40" }
axum   = "0.8"
tokio  = { version = "1", features = ["full"] }
serde  = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow  = "1"
async-trait = "0.1"
"#;
```

- [ ] **Step 2: Register the new template in Generator::new**

In `Generator::new`, add the dev template registration after the existing `cargo_toml` line (around line 49):

```rust
tera.add_raw_template("cargo_toml", CARGO_TOML_TEMPLATE)
    .unwrap();
tera.add_raw_template("cargo_toml_dev", CARGO_TOML_DEV_TEMPLATE)
    .unwrap();
```

- [ ] **Step 3: Update scaffold_project signature and logic**

Change the method signature and body of `scaffold_project` (around line 186). Replace the `Cargo.toml` generation block and add `.ravel-dev` writing:

```rust
/// Scaffold the initial project skeleton (used by `ravel new`).
///
/// When `dev` is true, Cargo.toml uses `path` dependencies
/// pointing to the local framework crates, and a `.ravel-dev`
/// marker file is written.
pub fn scaffold_project(&self, project_name: &str, dev: bool) -> Result<()> {
    let dirs = [
        "app/Http/Controllers",
        "app/Http/Middleware",
        "app/Http/Requests",
        "app/Models",
        "app/Jobs",
        "app/Services",
        "app/Providers",
        "bootstrap",
        "config",
        "database/migrations",
        "database/seeders",
        "routes",
        "storage",
        "tests",
        "src",
        "src/bin",
    ];

    for dir in dirs {
        self.ensure_dir(dir)?;
    }

    // Cargo.toml — choose template based on dev mode
    let cargo_template = if dev { "cargo_toml_dev" } else { "cargo_toml" };
    self.overwrite_file("Cargo.toml", &self.render(cargo_template, project_name)?)?;

    // src/main.rs
    self.overwrite_file("src/main.rs", &self.render("main_rs", project_name)?)?;

    // Default config file
    self.overwrite_file("config/app.toml", &self.render("app_toml", project_name)?)?;

    // Default .env
    if !self.exists(".env") {
        self.overwrite_file(".env", &self.render("env", project_name)?)?;
    }

    // .env.example (always overwrite to keep in sync)
    self.overwrite_file(".env.example", &self.render("env", project_name)?)?;

    // Dev marker file — records framework_root for ravel serve
    if dev {
        let framework_root = std::env::current_dir()
            .context("Failed to read current directory")?;
        let marker = format!(
            "framework_root = \"{}\"\n",
            framework_root.display()
        );
        self.overwrite_file(".ravel-dev", &marker)?;
    }

    // Database migrator (database/migrations/mod.rs)
    if !self.exists("database/migrations/mod.rs") {
        self.overwrite_file("database/migrations/mod.rs", MIGRATOR_TEMPLATE)?;
    }

    // Migrate binary (src/bin/migrate.rs)
    if !self.exists("src/bin/migrate.rs") {
        self.overwrite_file("src/bin/migrate.rs", MIGRATE_BIN_TEMPLATE)?;
    }

    // Seed binary (src/bin/seed.rs)
    if !self.exists("src/bin/seed.rs") {
        self.overwrite_file("src/bin/seed.rs", SEED_BIN_TEMPLATE)?;
    }

    Ok(())
}
```

- [ ] **Step 4: Update existing test for scaffold_project**

The existing test `test_scaffold_project` at line 725 calls `scaffold_project("MyApp")` — this needs a second argument. Change the call to:

```rust
g.scaffold_project("MyApp", false).unwrap();
```

- [ ] **Step 5: Add test for dev mode scaffold_project**

Add a new test after `test_scaffold_project`:

```rust
#[test]
fn test_scaffold_project_dev_mode() {
    let tmp = std::env::temp_dir().join("ravel_gen_project_dev");
    let _ = std::fs::remove_dir_all(&tmp);

    let g = Generator::new(&tmp);
    g.scaffold_project("MyApp", true).unwrap();

    assert!(tmp.join("Cargo.toml").exists());
    assert!(tmp.join("src/main.rs").exists());
    assert!(tmp.join(".ravel-dev").exists());

    // Dev mode Cargo.toml should use path dependencies
    let cargo = std::fs::read_to_string(tmp.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("path = \"../crates/ravel-core\""));
    assert!(cargo.contains("path = \"../crates/ravel-http\""));
    assert!(!cargo.contains("git = \"https://github.com"));

    // .ravel-dev should contain framework_root
    let marker = std::fs::read_to_string(tmp.join(".ravel-dev")).unwrap();
    assert!(marker.contains("framework_root"));

    let _ = std::fs::remove_dir_all(&tmp);
}
```

- [ ] **Step 6: Run generator tests**

```bash
cargo test -p ravel-generator
```

Expected: all existing tests + new dev mode test pass.

- [ ] **Step 7: Verify ravel-cli compiles**

```bash
cargo check -p ravel-cli
```

Expected: pass now that `scaffold_project` signature matches.

- [ ] **Step 8: Commit**

```bash
git add crates/ravel-generator/src/lib.rs
git commit -m "feat(generator): add dev mode — path deps template + .ravel-dev marker"
```

---

### Task 4: Make ravel serve workspace-aware

**Files:**
- Modify: `crates/ravel-cli/src/commands/serve.rs`

- [ ] **Step 1: Rewrite serve.rs with dev mode detection**

Replace the entire file:

```rust
//! ravel serve — start the development server.
//!
//! In normal mode: validates project structure, reads port from
//! `config/app.toml`, warns if the port is in use, then runs
//! `cargo run` in the current directory.
//!
//! In dev mode (detected via `.ravel-dev` marker): reads the
//! framework root from the marker and runs `cargo run -p <pkg>`
//! from the workspace root so path dependencies are resolved.

use anyhow::{Context, Result};
use std::net::TcpListener;
use std::path::Path;
use std::process::Command;

// ── Config reader ──────────────────────────────────────────────────

fn read_config() -> (u16, String) {
    let path = Path::new("config/app.toml");
    if !path.exists() {
        return (3000, "127.0.0.1".into());
    }
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (3000, "127.0.0.1".into()),
    };
    match toml::from_str::<toml::Value>(&content) {
        Ok(val) => {
            let port = val
                .get("server")
                .and_then(|s| s.get("port"))
                .and_then(|p| p.as_integer())
                .map(|p| p as u16)
                .unwrap_or(3000);
            let host = val
                .get("server")
                .and_then(|s| s.get("host"))
                .and_then(|h| h.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "127.0.0.1".into());
            (port, host)
        }
        Err(_) => (3000, "127.0.0.1".into()),
    }
}

fn is_port_in_use(host: &str, port: u16) -> bool {
    TcpListener::bind(format!("{host}:{port}")).is_err()
}

// ── Dev marker ─────────────────────────────────────────────────────

struct DevMarker {
    framework_root: String,
}

fn read_dev_marker() -> Option<DevMarker> {
    let path = Path::new(".ravel-dev");
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    let val: toml::Value = toml::from_str(&content).ok()?;
    let framework_root = val
        .get("framework_root")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())?;
    Some(DevMarker { framework_root })
}

// ── Main handler ───────────────────────────────────────────────────

pub fn handle() -> Result<()> {
    // Check for dev mode marker
    let dev = read_dev_marker();

    if let Some(ref dev_marker) = dev {
        let framework_root = Path::new(&dev_marker.framework_root);
        if !framework_root.join("Cargo.toml").exists() {
            eprintln!(
                "⚠️  Dev project marker references missing framework at {}",
                dev_marker.framework_root
            );
            eprintln!("   Falling back to normal mode.");
            return run_normal();
        }
        return run_dev(dev_marker);
    }

    run_normal()
}

fn run_normal() -> Result<()> {
    if !Path::new("Cargo.toml").exists() {
        anyhow::bail!("No Cargo.toml found. Are you in a Ravel project directory?");
    }

    let (port, host) = read_config();

    println!("🚀 Starting server...");
    println!("   http://{host}:{port}");

    if is_port_in_use(&host, port) {
        eprintln!();
        eprintln!("⚠️  Warning: Port {port} is already in use on {host}.");
        eprintln!("   Change it in config/app.toml or stop the other process.");
        eprintln!();
    }

    println!("   Press Ctrl+C to stop");
    println!();

    let status = Command::new("cargo").arg("run").status()?;

    if !status.success() {
        if let Some(code) = status.code()
            && code != 130
        {
            anyhow::bail!("Server exited with code {code}");
        }
    }

    Ok(())
}

fn run_dev(marker: &DevMarker) -> Result<()> {
    // Read project package name from Cargo.toml
    let cargo_toml_path = Path::new("Cargo.toml");
    let cargo_content = std::fs::read_to_string(cargo_toml_path)
        .context("Failed to read Cargo.toml")?;
    let cargo_val: toml::Value = toml::from_str(&cargo_content)
        .context("Failed to parse Cargo.toml")?;
    let pkg_name = cargo_val
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("my_app");

    let (port, host) = read_config();

    println!("🚀 Starting server (dev mode)...");
    println!("   Framework: {}", marker.framework_root);
    println!("   http://{host}:{port}");
    println!();

    let status = Command::new("cargo")
        .arg("run")
        .arg("-p")
        .arg(pkg_name)
        .current_dir(&marker.framework_root)
        .status()?;

    if !status.success() {
        if let Some(code) = status.code()
            && code != 130
        {
            anyhow::bail!("Server exited with code {code}");
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Verify ravel-cli compiles**

```bash
cargo check -p ravel-cli
```

Expected: pass.

- [ ] **Step 3: Commit**

```bash
git add crates/ravel-cli/src/commands/serve.rs
git commit -m "feat(serve): detect .ravel-dev marker and run from workspace root"
```

---

### Task 5: Clean up old MyApp and verify end-to-end

**Files:**
- Delete: `MyApp/` (the old git-deps version)

- [ ] **Step 1: Remove old MyApp directory**

```bash
git rm -r MyApp/
```

- [ ] **Step 2: Commit the removal**

```bash
git commit -m "chore: remove old git-deps MyApp (replaced by ravel new --dev)"
```

- [ ] **Step 3: Full workspace build check**

```bash
cargo build --workspace
```

Expected: all crates build successfully.

- [ ] **Step 4: Run all tests**

```bash
cargo test --workspace
```

Expected: all tests pass.

- [ ] **Step 5: Manual end-to-end verification**

Create a dev project and verify the full loop:

```bash
# From workspace root
cargo run -p ravel-cli -- new my-app --dev

# Check generated Cargo.toml uses path deps
cat my-app/Cargo.toml | grep "path ="

# Check .ravel-dev exists
cat my-app/.ravel-dev

# Build from workspace root (this is what serve does)
cargo build -p my_app
```

Expected: project created with path deps, `.ravel-dev` present, builds successfully.

- [ ] **Step 6: Commit any final adjustments**

```bash
git add -A
git commit -m "chore: final verification of dev mode workflow"
```

---

### Task 6 (Optional): Add CLI integration tests

**Files:**
- Create: `crates/ravel-cli/tests/cli_tests.rs`

If time permits, add an integration test for the `--dev` flag validation:

- [ ] **Step 1: Create test file**

```rust
use std::process::Command;

#[test]
fn test_new_dev_outside_framework_fails() {
    // Run from a temp directory that is NOT the framework root
    let tmp = std::env::temp_dir();
    let output = Command::new("cargo")
        .args(["run", "-p", "ravel-cli", "--", "new", "TestApp", "--dev"])
        .current_dir(&tmp)
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected failure when using --dev outside framework"
    );
    assert!(
        stderr.contains("--dev can only be used inside the Ravel framework repository"),
        "Expected specific error message, got: {stderr}"
    );
}
```

- [ ] **Step 2: Run the integration test**

```bash
cargo test -p ravel-cli
```

- [ ] **Step 3: Commit**

```bash
git add crates/ravel-cli/tests/
git commit -m "test(cli): add integration test for --dev outside framework"
```

---

### Task 7: Add .superpowers/ and my-app/ to .gitignore

**Files:**
- Modify: `.gitignore`

- [ ] **Step 1: Add entries to .gitignore**

Append to `.gitignore`:

```
# Superpowers brainstorming artifacts
.superpowers/

# Dev-mode test project (created via ravel new --dev)
my-app/
```

- [ ] **Step 2: Commit**

```bash
git add .gitignore
git commit -m "chore: gitignore .superpowers/ and dev my-app/"
```
