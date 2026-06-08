//! ravel serve — start the development server.
//!
//! 1. Builds the project via `cargo build`
//! 2. Spawns the binary
//! 3. Waits for it to exit (Ctrl+C)

use anyhow::Result;
use std::process::Command;

pub fn handle() -> Result<()> {
    println!("🔨 Building...");
    let build = Command::new("cargo")
        .args(["build"])
        .status()?;

    if !build.success() {
        anyhow::bail!("Build failed — check errors above");
    }

    println!("🚀 Starting server...");

    // Run the built binary
    let status = Command::new("cargo")
        .args(["run"])
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
