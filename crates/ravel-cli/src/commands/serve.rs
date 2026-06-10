//! ravel serve — start the development server.
//!
//! Validates the project structure, reads the configured port
//! from `config/app.toml`, warns if the port is in use,
//! then runs `cargo run`.

use anyhow::Result;
use std::net::TcpListener;
use std::path::Path;
use std::process::Command;

fn read_config() -> (u16, String) {
    let path = Path::new("config/app.toml");
    if !path.exists() {
        return (3000, "127.0.0.1".into());
    }
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (3000, "127.0.0.1".into()),
    };
    // Parse server section; if anything fails, use defaults
    match toml::from_str::<toml::Value>(&content) {
        Ok(val) => {
            let port = val.get("server")
                .and_then(|s| s.get("port"))
                .and_then(|p| p.as_integer())
                .map(|p| p as u16)
                .unwrap_or(3000);
            let host = val.get("server")
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

pub fn handle() -> Result<()> {
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
