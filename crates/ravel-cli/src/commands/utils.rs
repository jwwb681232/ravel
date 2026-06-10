use std::path::Path;

/// Check whether a cargo bin target named `name` exists.
///
/// Looks for `src/bin/<name>.rs` or an explicit `[[bin]]` entry in Cargo.toml.
pub(crate) fn find_bin(name: &str) -> Option<String> {
    let bin_path = format!("src/bin/{name}.rs");
    if Path::new(&bin_path).exists() {
        return Some(name.to_string());
    }

    let cargo_path = Path::new("Cargo.toml");
    if !cargo_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(cargo_path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name = \"") {
            if let Some(quoted) = trimmed.strip_prefix("name = \"")
                && let Some(quoted_name) = quoted.strip_suffix('"')
            {
                if quoted_name == name {
                    return Some(name.to_string());
                }
            }
        }
    }

    None
}
