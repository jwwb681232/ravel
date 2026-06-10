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
        // Skip comment-only lines
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if !trimmed.contains('"') {
            continue;
        }
        for method in &methods {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_get() {
        let src = r#"Route::get("/", handler)"#;
        let routes = parse_routes(src);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].0, "GET");
        assert_eq!(routes[0].1, "/");
    }

    #[test]
    fn test_parse_multiple_methods() {
        let src = r#"
            Route::get("/", handler);
            Route::post("/users", create);
            Route::delete("/users/{id}", destroy);
        "#;
        let routes = parse_routes(src);
        assert_eq!(routes.len(), 3);
        assert_eq!(routes[0], ("GET".into(), "/".into()));
        assert_eq!(routes[1], ("POST".into(), "/users".into()));
        assert_eq!(routes[2], ("DELETE".into(), "/users/{id}".into()));
    }

    #[test]
    fn test_parse_builder_pattern() {
        let src = r#"
            let router = Route::new()
                .get("/api/health", health)
                .post("/api/login", login);
        "#;
        let routes = parse_routes(src);
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].1, "/api/health");
        assert_eq!(routes[1].1, "/api/login");
    }

    #[test]
    fn test_parse_grouped_routes() {
        let src = r#"
            Route::group("/admin", || {
                Route::get("/dashboard", admin_dashboard);
                Route::get("/users", admin_users);
            });
        "#;
        let routes = parse_routes(src);
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].1, "/dashboard");
        assert_eq!(routes[1].1, "/users");
    }

    #[test]
    fn test_parse_empty_file() {
        assert!(parse_routes("").is_empty());
        assert!(parse_routes("// no routes here").is_empty());
    }

    #[test]
    fn test_parse_commented_routes() {
        let src = r#"
            // Route::get("/old", old_handler);
            Route::get("/new", new_handler);
        "#;
        let routes = parse_routes(src);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].1, "/new");
    }
}
