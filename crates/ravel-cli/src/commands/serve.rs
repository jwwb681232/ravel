//! ravel serve — start the development server.
//!
//! Checks project structure, reads the configured port from `config/app.toml`,
//! warns if the port is already in use, then runs `cargo run`.

use anyhow::Result;
use std::net::TcpListener;
use std::path::Path;
use std::process::Command;

/// Read the server port from `config/app.toml` if it exists.
fn read_port_from_config() -> Option<u16> {
    let path = Path::new("config/app.toml");
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("port = ") {
            let val = trimmed.strip_prefix("port = ")?;
            return val.parse().ok();
        }
    }
    None
}

/// Check whether a port is already in use.
fn is_port_in_use(port: u16) -> bool {
    TcpListener::bind(format!("127.0.0.1:{}", port)).is_err()
}

pub fn handle() -> Result<()> {
    // Validate project structure
    if !Path::new("Cargo.toml").exists() {
        anyhow::bail!("No Cargo.toml found. Are you in a Ravel project directory?");
    }

    let port = read_port_from_config().unwrap_or(3000);

    println!("🚀 Starting server...");
    println!("   Port: {}", port);

    if is_port_in_use(port) {
        eprintln!();
        eprintln!("⚠️  Warning: Port {} is already in use.", port);
        eprintln!("   Change the port in config/app.toml or stop the other process.");
        eprintln!();
    }

    println!("   Press Ctrl+C to stop");
    println!();

    let status = Command::new("cargo").arg("run").status()?;

    if !status.success() {
        // ctrl-c gives non-zero; that's fine
        if let Some(code) = status.code()
            && code != 130
        {
            anyhow::bail!("Server exited with code {code}");
        }
    }

    Ok(())
}
