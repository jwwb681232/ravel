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
        .context("Cargo.toml is missing [package] name field")?;

    // Read config from the project directory (current dir),
    // not from framework_root — the project owns its own config
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
