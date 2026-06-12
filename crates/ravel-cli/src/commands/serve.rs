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

/// Read bind address from `config/app.toml` (and the local `.env`).
///
/// This mirrors what `ravel_core::app::Application::load_config` does,
/// so the "plan" printed by `ravel serve` matches what the child
/// process will actually bind to — including any `APP_*` env var
/// overrides and any values in the project's `.env`.
fn read_config() -> (u16, String) {
    // Use the framework's own loader so we don't duplicate the
    // env-resolution logic.
    let dir = Path::new("config");
    if !dir.is_dir() {
        return (3000, "127.0.0.1".into());
    }

    // Populate process env from `.env` (no override — real shell env wins).
    let env_path = dir.parent().unwrap_or(dir).join(".env");
    let _ = dotenvy::from_path(&env_path);

    let env: std::collections::HashMap<String, String> = std::env::vars().collect();

    let Ok(mut repo) = ravel_core::config::ConfigRepo::load_dir(dir) else {
        return (3000, "127.0.0.1".into());
    };
    let _ = repo.resolve_env_overrides(&env);

    let port: u16 = repo
        .get_or("app.server.port", 3000u16);
    let host: String = repo
        .get_or("app.server.host", "127.0.0.1".to_string());
    (port, host)
}

fn is_port_in_use(host: &str, port: u16) -> bool {
    TcpListener::bind(format!("{host}:{port}")).is_err()
}

// ── Dev marker ─────────────────────────────────────────────────────

struct DevMarker {
    framework_root: String,
    /// Path to the scaffolded project (where `.env` and `config/` live).
    /// Optional for backward compat with markers written by older
    /// versions of the CLI.
    project_root: Option<String>,
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
    let project_root = val
        .get("project_root")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Some(DevMarker {
        framework_root,
        project_root,
    })
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
        .context("Cargo.toml is missing [package] name field")?;

    // Read config from the project directory (current dir),
    // not from framework_root — the project owns its own config
    let (port, host) = read_config();

    println!("🚀 Starting server (dev mode)...");
    println!();

    // Load the project's `.env` into *our* process environment. The
    // spawned `cargo run` then inherits it, so the child binary finds
    // APP_KEY etc. without needing dotenvy to be re-invoked from
    // framework_root (where it would look for a non-existent `.env`).
    if let Some(project_root) = &marker.project_root {
        let env_path = Path::new(project_root).join(".env");
        // Use `from_path` (not `from_path_override`) so that the user's
        // shell env still wins — matches Laravel semantics.
        let _ = dotenvy::from_path(&env_path);
    }

    let status = Command::new("cargo")
        .arg("run")
        .arg("-p")
        .arg(pkg_name)
        .arg("--bin")
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
