//! ravel route:list — display all registered routes.
//!
//! Scans the `routes/` directory for route definitions like
//! `.get("/path", handler)`, `.post("/path", handler)`, etc.

use anyhow::Result;
use std::fs;
use std::path::Path;

pub fn handle() -> Result<()> {
    println!("📋 Registered Routes");
    println!("{:-<60}", "");

    let routes_dir = Path::new("routes");

    if routes_dir.is_dir() {
        let mut found = false;
        let mut entries: Vec<_> = fs::read_dir(routes_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in &entries {
            let path = entry.path();
            let name = path.file_stem().unwrap().to_str().unwrap_or("unknown");
            let content = fs::read_to_string(&path)?;

            let file_routes = parse_routes(&content);
            if !file_routes.is_empty() {
                found = true;
                println!("  📄 routes/{}.rs", name);
                for (method, route_path) in &file_routes {
                    println!("     {} {}", method, route_path);
                }
            }
        }

        if !found {
            println!("  (no route definitions found in routes/)");
        }
    } else {
        println!("  (no routes/ directory found — run `ravel new` first)");
    }

    println!();
    println!("💡 Tip: Use Route::new().get(\"/\", handler) to define routes.");

    Ok(())
}

/// Parse route definitions from source code.
///
/// Looks for patterns like:
/// - `.get("/path", handler)`
/// - `.post("/path", handler)`
/// - `.put("/path", handler)`
/// - `.delete("/path", handler)`
/// - `.patch("/path", handler)`
fn parse_routes(source: &str) -> Vec<(String, String)> {
    let mut routes = Vec::new();
    let methods = ["get", "post", "put", "delete", "patch"];

    for line in source.lines() {
        let trimmed = line.trim();
        for method in &methods {
            // Match `.method("/path", ...)`
            if trimmed.starts_with(&format!(".{}(\"", method))
                && let Some(path) = extract_path(trimmed, method)
            {
                routes.push((method.to_uppercase(), path));
            }
        }
    }

    routes
}

/// Extract the path string from a route definition line.
fn extract_path(line: &str, method: &str) -> Option<String> {
    let prefix = format!(".{}(\"", method);
    let start = line.find(&prefix)? + prefix.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
