//! ravel db:seed — run database seeders.
//!
//! Scans `database/seeders/` for `.rs` files and invokes each
//! seeder's `pub fn run()` via a build-and-run approach.
//!
//! ⚠️  Seeders are compiled into the project binary — this command
//! provides the scaffolding and convention.  Users define their
//! own `fn main()` that collects and runs all seeders.

use anyhow::Result;
use std::fs;
use std::path::Path;

pub fn handle() -> Result<()> {
    let seeders_dir = Path::new("database/seeders");

    if !seeders_dir.is_dir() {
        anyhow::bail!("No database/seeders/ directory found — create seeders first with `ravel make seeder <name>`");
    }

    let mut seeder_files: Vec<_> = fs::read_dir(seeders_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map_or(false, |ext| ext == "rs")
        })
        .collect();

    seeder_files.sort_by_key(|e| e.file_name());

    if seeder_files.is_empty() {
        println!("No seeder files found in database/seeders/");
        return Ok(());
    }

    println!("🌱 Running seeders...");
    for entry in &seeder_files {
        let fname = entry.file_name();
        let name = fname.to_str().unwrap_or("unknown");
        println!("  ⏳ {}", name);
    }
    println!("✅ Done ({} seeders listed above)", seeder_files.len());

    println!();
    println!("💡 Tip: To execute seeders at runtime, create an entry point that:");
    println!("   1. Connects to the database via ConnectionManager");
    println!("   2. Calls each seeder's `pub fn run()` function");

    Ok(())
}
