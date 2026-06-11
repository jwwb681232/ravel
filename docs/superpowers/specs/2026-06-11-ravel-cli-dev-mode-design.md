# Ravel CLI Dev Mode: Framework Developer Dogfooding

**Date:** 2026-06-11
**Status:** approved

## 1. Problem

As the Ravel framework developer, you need to simultaneously:

- Develop framework crates (`ravel-http`, `ravel-core`, etc.) in the workspace
- Use those crates in a test application (`my-app`) to verify they work
- Use `ravel-cli` commands (`serve`, `make:*`, `migrate`, etc.) against that test app

Three pain points exist today:

| Pain | Current behavior | Impact |
|------|------------------|--------|
| Git dependencies | `ravel new` generates `Cargo.toml` with `git = "https://github.com/..."` | Must `git push` + `cargo update` for every framework change |
| CLI staleness | `cargo install ravel-cli` produces a static binary | Must re-install after every CLI code change |
| No dev awareness | `ravel serve` is a thin `cargo run` wrapper | No workspace context, no auto-rebuild of framework crates |

Goal: a `--dev` flag on `ravel new` that creates a project wired to the local framework source, enabling a fast edit-build-test cycle without git push.

## 2. Design

### 2.1 `ravel new --dev`

**Entry point:** `ravel-cli/src/main.rs`

Add `--dev` flag to the `New` subcommand:

```rust
New {
    name: String,
    /// Create a dev-mode project with path dependencies to the local framework
    #[arg(long)]
    dev: bool,
}
```

**Validation:** If `--dev` is passed, verify the current directory is the Ravel framework root. Check by looking for `crates/ravel-cli/Cargo.toml` relative to cwd. If not found, error:

```
Error: --dev can only be used inside the Ravel framework repository
```

**Behavior when `--dev`:**

1. Generate `Cargo.toml` with `path` dependencies instead of `git`:
   ```toml
   [dependencies]
   ravel-core     = { path = "../crates/ravel-core" }
   ravel-http     = { path = "../crates/ravel-http" }
   ravel-eloquent = { path = "../crates/ravel-eloquent" }
   ravel-facades  = { path = "../crates/ravel-facades" }
   ravel-db-seaorm = { path = "../crates/ravel-db-seaorm" }
   ```
2. Generate a `.ravel-dev` marker file in the project root:
   ```toml
   framework_root = "/absolute/path/to/ravel"
   ```
3. The project is placed inside the framework repo (e.g., `ravel/my-app/`)

**Behavior when NOT `--dev`:** Unchanged — generates `git` dependencies, works from any directory.

### 2.2 `.ravel-dev` marker file

Located at the project root (`my-app/.ravel-dev`). TOML format:

```toml
framework_root = "/home/dev/ravel"
```

`framework_root` records the absolute path to the workspace root at creation time. `ravel serve` reads this to discover where to run `cargo`.

### 2.3 `ravel serve` — dev-aware startup

Modified flow in `ravel-cli/src/commands/serve.rs`:

```
1. Check for .ravel-dev in current directory
   ├─ Found → dev mode (go to step 2)
   └─ Not found → normal mode (cargo run in cwd, current behavior)

2. Read framework_root from .ravel-dev
   ├─ Exists & has Cargo.toml → cd to framework_root
   │   cargo run -p my-app    (from workspace context)
   └─ Missing/invalid → warn, fall back to normal mode
```

In dev mode, `cargo run -p <package_name>` runs from the workspace root. Cargo's incremental compilation ensures only changed crates are rebuilt — no full rebuild every time.

### 2.4 CLI invocation during development

No wrapper script or cargo alias. Use `cargo run -p ravel-cli --` directly from the workspace root:

```bash
# Initial setup (one-time)
cargo run -p ravel-cli -- new my-app --dev

# Daily development
cargo run -p ravel-cli -- serve
cargo run -p ravel-cli -- make:controller UserController
cargo run -p ravel-cli -- migrate
```

This always runs the latest code, no re-install needed.

### 2.5 Workspace Cargo.toml

Remove `exclude = ["MyApp"]` from the workspace `Cargo.toml`. Since `my-app` will use `path` dependencies pointing into the workspace, it can coexist as a workspace member. The workspace `members` list stays as-is (`crates/*` only) — `ravel serve` runs `cargo run -p my-app` from the workspace root, so cargo resolves the package via the workspace manifest.

The previous `MyApp/` directory (with git deps) should be removed or renamed.

### 2.6 `ravel-generator` changes

`Generator::scaffold_project` gains a `dev: bool` parameter. When `true`:

- The Cargo.toml template renders `path` dependencies instead of `git`
- A `.ravel-dev` file is written alongside the generated project
- The `framework_root` in `.ravel-dev` uses `std::env::current_dir()` canonicalized at scaffold time

The generator already uses Tera templates (or string templates). Two template variants for the `[dependencies]` section in Cargo.toml — one for git, one for path.

## 3. Error Handling

| Scenario | Behavior |
|----------|----------|
| `ravel new foo --dev` outside framework | Error: "--dev can only be used inside the Ravel framework repository" |
| `ravel serve` in non-ravel directory | Current behavior: "No Cargo.toml found" |
| `ravel serve` with stale `.ravel-dev` (framework_root deleted/moved) | Warn: "Dev project marker references missing framework at X. Falling back to normal mode." Then `cargo run` in cwd |
| `ravel new my-app --dev` when `my-app/` exists | Current behavior: error, directory exists |

## 4. Files Changed

| # | File | Change |
|---|------|--------|
| 1 | `crates/ravel-cli/src/main.rs` | Add `--dev` flag to `New` subcommand |
| 2 | `crates/ravel-cli/src/commands/new.rs` | Pass `dev` to Generator; validate inside framework |
| 3 | `crates/ravel-cli/src/commands/serve.rs` | Read `.ravel-dev`; workspace-aware startup |
| 4 | `crates/ravel-generator/src/lib.rs` | `scaffold_project` accepts `dev: bool`; dual templates |
| 5 | `Cargo.toml` (workspace root) | Remove `exclude = ["MyApp"]` |

## 5. Non-Goals

- `--dev` is NOT for end users. It is a framework-developer-only tool.
- Does not change `make:*`, `migrate`, `key:generate`, or other CLI commands — they already work from the project directory and don't care about dependency source.
- Does not introduce hot-reload or file watching beyond cargo's incremental compilation.
- Does not support creating `--dev` projects outside the framework repo.
