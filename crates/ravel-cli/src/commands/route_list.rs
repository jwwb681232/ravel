//! ravel route:list — display all registered routes.
//!
//! Since Rust route registration is compile-time, this command
//! helps the user by scanning the route definitions in the project.

use anyhow::Result;
use std::fs;
use std::path::Path;

pub fn handle() -> Result<()> {
    println!("📋 Registered Routes");
    println!("{:-<60}", "");

    // Scan routes/ directory for .rs route files
    let routes_dir = Path::new("routes");

    if routes_dir.is_dir() {
        for entry in fs::read_dir(routes_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "rs") {
                let name = path.file_stem().unwrap().to_str().unwrap_or("unknown");
                println!("  📄 {}", name);
            }
        }
    } else {
        println!("  (no routes/ directory found — run `ravel new` first)");
    }

    println!();
    println!("💡 Tip: Open src/main.rs or your route files to see full route definitions.");
    println!("    Use `Route::new().get(\"/\", handler)` to define routes.");

    Ok(())
}
