use anyhow::{Context, Result, bail};
use ravel_generator::Generator;
use std::path::Path;

/// Check whether the current directory is the Ravel framework root.
/// The framework root contains `crates/ravel-cli/Cargo.toml`.
fn is_framework_root() -> bool {
    Path::new("crates/ravel-cli/Cargo.toml").exists()
}

/// Add a member to the workspace Cargo.toml members array.
fn add_workspace_member(project_name: &str) -> Result<()> {
    let path = Path::new("Cargo.toml");
    let content = std::fs::read_to_string(path)
        .context("Failed to read workspace Cargo.toml")?;
    // Normalize CRLF → LF for consistent handling
    let content = content.replace("\r\n", "\n");

    let member_entry = format!("\"{}\"", project_name);

    // Already a member — nothing to do
    if content.contains(&member_entry) {
        return Ok(());
    }

    // Insert before the closing `]` of the members array.
    let new_content = if let Some(pos) = content.rfind("\n]") {
        let (before, after) = content.split_at(pos);
        // Avoid double-comma when the last entry already has a trailing comma
        let comma = if before.ends_with(',') { "" } else { "," };
        format!("{}{}\n    {}\n]{}", before, comma, member_entry, &after[2..])
    } else {
        bail!("Could not find members array closing bracket in workspace Cargo.toml");
    };

    std::fs::write(path, new_content)
        .context("Failed to update workspace Cargo.toml")?;

    println!("   Added `{}` to workspace members", project_name);
    Ok(())
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

    // Dev mode: register in workspace so cargo works from project dir
    if dev {
        add_workspace_member(project_name)?;
    }

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
