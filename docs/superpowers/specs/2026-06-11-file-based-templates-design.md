# File-Based Templates for `ravel new`

**Date**: 2026-06-11  
**Status**: Approved  
**Scope**: `crates/ravel-cli/src/generator.rs`

## Problem

The `ravel new` command generates project files from 29 inline string constants
embedded in `generator.rs`. This makes the file 1262 lines long, most of which
is template content. Developers cannot get syntax highlighting or formatting
for the embedded code, and any template change requires navigating a wall of
`r#"..."#` strings.

## Goal

Extract all templates into standalone files under `templates/`, loaded at
compile time via `include_dir!`, the same approach used by Tauri's CLI.

## Design

### Directory layout

```
crates/ravel-cli/
├── src/
│   └── generator.rs          ← rendering logic only (~300 lines)
└── templates/
    ├── controller.rs
    ├── middleware.rs
    ├── migration.rs
    ├── seeder.rs
    ├── provider.rs
    ├── request.rs
    ├── model.rs
    ├── job.rs
    ├── main.rs
    ├── app.toml
    ├── cargo-release.toml
    ├── cargo-dev.toml
    ├── env
    ├── database.toml
    ├── migrator.rs
    ├── migrate-bin.rs
    ├── seed-bin.rs
    ├── routes-web.rs
    ├── user-model.rs
    ├── post-model.rs
    ├── user-controller.rs
    ├── post-controller.rs
    ├── create-post-request.rs
    ├── send-welcome-job.rs
    ├── app-service-provider.rs
    ├── route-service-provider.rs
    ├── migration-users.rs
    ├── migration-posts.rs
    └── user-seeder.rs
```

Every template currently stored as a `const …_TEMPLATE: &str` becomes a file
at `templates/<name>` with the same content (minus the Rust string escaping).

### File naming convention

- Files use their **target extension** (`.rs`, `.toml`, or no extension for `.env`).
- This is intuitive — the filename already tells you what it generates.
- `templates/` is outside `src/`, so rust-analyzer won't flag `{{name}}` as a syntax error.

### Loading mechanism

```rust
use include_dir::{include_dir, Dir};

static TEMPLATES: Dir = include_dir!("$CARGO_MANIFEST_DIR/templates");
```

One line embeds all 29 files at compile time. At runtime the generator looks
up templates by name and renders them with Tera's `render_str()` (no
pre-registration needed).

### Render path

Before:
```rust
// Register
tera.add_raw_template("controller", CONTROLLER_TEMPLATE).unwrap();
// Render
let content = self.render("controller", name)?;
```

After:
```rust
let file = TEMPLATES.get_file("controller.rs")?;
let src = file.contents_utf8().unwrap();
let rendered = self.tera.render_str(src, &ctx)?;
self.create_file(&output_path, &rendered)?;
```

### App key handling

The `env` template still uses `{{app_key}}`. The key is generated in
`scaffold_project()` and injected into the Tera context before rendering the
`env` template. This logic does not change.

### What stays the same

- Tera as the template engine (variables: `{{name}}`, `{{snake}}`, `{{kebab}}`, `{{timestamp}}`, `{{app_key}}`)
- `make_context()` helper  
- All scaffolding method signatures (`scaffold_controller`, `scaffold_model`, etc.)
- Test coverage (the same assertions, paths change only if they reference templates directly)

### Dependencies

Add to `crates/ravel-cli/Cargo.toml`:
```toml
include_dir = "0.7"
```

No other dependency changes.

### Removal

After migration, delete:
- All 29 `const …_TEMPLATE: &str` constants from `generator.rs`
- All `tera.add_raw_template(…)` calls from `Generator::new()`

## Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| Template file not found at compile time | `include_dir!` fails at compile time, not runtime |
| rust-analyzer flags `{{name}}` in `.rs` templates | `templates/` is outside `src/`, ignored by default |
| `include_dir` doesn't support no-extension files | It does — `Dir` stores arbitrary files keyed by relative path |

## Test Plan

- All 17 existing `ravel-cli` tests must pass without changes to their assertion
  logic (paths remain `app/…`, `routes/…`, `database/…`).
- Manually verify `ravel new my-app` produces identical output to the pre-migration version.
