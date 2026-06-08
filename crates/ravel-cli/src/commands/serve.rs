//! ravel serve — start the development server.
//!
//! Runs `cargo run` which builds and starts the project binary.

use anyhow::Result;
use std::process::Command;

pub fn handle() -> Result<()> {
    println!("🚀 Starting server...");

    let status = Command::new("cargo")
        .arg("run")
        .status()?;

    if !status.success() {
        // ctrl-c gives non-zero; that's fine
        if let Some(code) = status.code() {
            if code != 130 {
                anyhow::bail!("Server exited with code {code}");
            }
        }
    }

    Ok(())
}
