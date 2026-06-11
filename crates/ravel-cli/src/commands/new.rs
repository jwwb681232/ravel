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
