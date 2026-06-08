use anyhow::{bail, Result};
use ravel_generator::Generator;
use std::path::Path;

pub fn handle(project_name: &str) -> Result<()> {
    let root = Path::new(project_name);

    if root.exists() {
        bail!("Directory '{}' already exists", project_name);
    }

    let g = Generator::new(root);
    g.scaffold_project(project_name)?;

    println!("✅ Project created: {}", project_name);
    println!();
    println!("  cd {}", project_name);
    println!("  ravel serve");

    Ok(())
}
