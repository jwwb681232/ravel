//! Path helpers — project directory path builders.
use std::path::PathBuf;

fn project_root() -> &'static PathBuf {
    use std::sync::OnceLock;
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| std::env::current_dir().expect("Failed to determine project root"))
}

pub fn base_path(segments: &[&str]) -> PathBuf {
    let mut p = project_root().clone();
    for seg in segments {
        p.push(seg);
    }
    p
}

pub fn config_path(file: &str) -> PathBuf {
    project_root().join("config").join(file)
}

pub fn database_path(file: &str) -> PathBuf {
    project_root().join("database").join(file)
}

pub fn storage_path(file: &str) -> PathBuf {
    project_root().join("storage").join(file)
}
