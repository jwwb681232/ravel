//! ravel route:list — display all registered routes.
//!
//! Scans the `routes/` directory for route source files and prints
//! them in a human-readable table.  Routes are also available in-process
//! via `ravel_facades::Route::list()` (useful for health-check endpoints).

use anyhow::Result;
use std::fs;
use std::path::Path;

pub fn handle() -> Result<()> {
    let routes_dir = Path::new("routes");

    if !routes_dir.is_dir() {
        println!("📋 Registered Routes");
        println!("{:-<60}", "");
        println!("  (no routes/ directory found)");
        println!();
        println!("💡 Define routes in routes/web.rs using:");
        println!("   Route::get(\"/\", || async {{ \"Hello\" }});");
        println!();
        println!("💡 For in-process introspection, use:");
        println!("   ravel_facades::Route::list()  // returns Vec<RouteEntry>");
        return Ok(());
    }

    let mut entries: Vec<_> = fs::read_dir(routes_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    println!("📋 Registered Routes");
    println!("{:-<60}", "");

    let mut total = 0;
    for entry in &entries {
        let path = entry.path();
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("?");
        let content = fs::read_to_string(&path)?;

        let file_routes = parse_routes(&content);
        if !file_routes.is_empty() {
            println!("  📄 routes/{name}.rs");
            for (method, uri) in &file_routes {
                println!("     {method:8} {uri}");
                total += 1;
            }
        }
    }

    if total == 0 {
        println!("  (no route calls found)");
    } else {
        println!();
        println!("   Total: {total} route{}", if total == 1 { "" } else { "s" });
    }

    Ok(())
}

// ── Lightweight source parser ─────────────────────────────────────────

fn parse_routes(source: &str) -> Vec<(String, String)> {
    let mut routes = Vec::new();
    let methods = ["get", "post", "put", "delete", "patch"];
    for line in source.lines() {
        let trimmed = line.trim();
        if !trimmed.contains('"') {
            continue;
        }
        for method in &methods {
            // Match: .get("/path"  or  ::get("/path"  or  Route::get("/path"
            for prefix in &[format!(".{method}(\""), format!("::{method}(\"")] {
                if let Some(pos) = trimmed.find(prefix) {
                    let rest = &trimmed[pos + prefix.len()..];
                    if let Some(end) = rest.find('"') {
                        let uri = rest[..end].to_string();
                        routes.push((method.to_uppercase(), uri));
                        break;
                    }
                }
            }
        }
    }
    routes
}
