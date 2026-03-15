use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Get the path to the `.lazypr` directory for a given repo root.
pub fn store_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".lazypr")
}

/// Initialize the `.lazypr` state directory. Idempotent -- safe to call on every startup.
///
/// Creates: `.lazypr/`, `.lazypr/reviews/`, `.lazypr/cache/`, `.lazypr/stats/`
pub fn init_store(repo_root: &Path) -> Result<PathBuf> {
    let base = store_path(repo_root);
    for dir in &["reviews", "cache", "stats"] {
        std::fs::create_dir_all(base.join(dir))
            .with_context(|| format!("failed to create .lazypr/{}", dir))?;
    }
    Ok(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn store_path_returns_correct_path() {
        let root = Path::new("/tmp/my-repo");
        assert_eq!(store_path(root), PathBuf::from("/tmp/my-repo/.lazypr"));
    }

    #[test]
    fn init_store_creates_all_directories() {
        let tmp = TempDir::new().expect("create temp dir");
        let base = init_store(tmp.path()).expect("init_store");

        assert!(base.exists());
        assert!(base.join("reviews").is_dir());
        assert!(base.join("cache").is_dir());
        assert!(base.join("stats").is_dir());
    }

    #[test]
    fn init_store_is_idempotent() {
        let tmp = TempDir::new().expect("create temp dir");

        let first = init_store(tmp.path()).expect("first call");
        let second = init_store(tmp.path()).expect("second call");

        assert_eq!(first, second);
        assert!(first.join("reviews").is_dir());
        assert!(first.join("cache").is_dir());
        assert!(first.join("stats").is_dir());
    }
}
