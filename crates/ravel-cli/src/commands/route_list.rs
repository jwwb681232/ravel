//! ravel route:list — display all registered routes.
//!
//! Scans the `routes/` directory for route definitions. Supports both
//! `ravel_http::route::Route` builder syntax and `ravel_facades::Route`
//! facade syntax.

use anyhow::Result;
use std::fs;
use std::path::Path;

pub fn handle() -> Result<()> {
    println!("📋 Registered Routes");
    println!("{:-<60}", "");

    let routes_dir = Path::new("routes");

    if !routes_dir.is_dir() {
        println!("  (no routes/ directory found — run `ravel new` first)");
        println!();
        return Ok(());
    }

    let mut entries: Vec<_> = fs::read_dir(routes_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            p.extension().is_some_and(|ext| ext == "rs")
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut total = 0;
    let mut found = false;

    for entry in &entries {
        let path = entry.path();
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .unwrap_or("unknown");
        let content = fs::read_to_string(&path)?;

        let file_routes = parse_routes(&content);
        if !file_routes.is_empty() {
            found = true;
            println!("  📄 routes/{}.rs", name);
            for (method, route_path) in &file_routes {
                println!("     {:8} {}", method, route_path);
                total += 1;
            }
        }
    }

    if !found {
        println!("  (no route definitions found in routes/)");
    } else {
        println!();
        println!("   Total: {} route{}", total, if total == 1 { "" } else { "s" });
    }

    println!();
    println!("💡 Tip: Use Route::get(\"/\", handler) or Route::new().get(\"/\", handler) to define routes.");

    Ok(())
}

// ── Route parsing ─────────────────────────────────────────────────────

fn parse_routes(source: &str) -> Vec<(String, String)> {
    let mut routes = Vec::new();
    let methods = ["get", "post", "put", "delete", "patch"];

    for line in source.lines() {
        let trimmed = line.trim();
        for method in &methods {
            if let Some(path) = extract_path(trimmed, method) {
                routes.push((method.to_uppercase(), path));
            }
        }
    }

    routes
}

/// Extract a route path from a line of source code.
///
/// Matches both builder pattern:
///   - `.get("/path", handler)`
///   - `admin.get("/path", handler)`
///
/// And facade / type pattern:
///   - `Route::get("/path", handler)`
///   - `ravel_facades::Route::get("/path", handler)`
fn extract_path(line: &str, method: &str) -> Option<String> {
    // Try builder pattern: any `.get("...",` anywhere in the line
    let dot_prefix = format!(".{method}(\"");
    if let Some(path) = extract_path_with_prefix(line, &dot_prefix) {
        return Some(path);
    }

    // Try facade / type pattern: any `::get("...",` anywhere in the line
    let colons_prefix = format!("::{method}(\"");
    if let Some(path) = extract_path_with_prefix(line, &colons_prefix) {
        return Some(path);
    }

    None
}

fn extract_path_with_prefix(line: &str, prefix: &str) -> Option<String> {
    let start = line.find(prefix)? + prefix.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
