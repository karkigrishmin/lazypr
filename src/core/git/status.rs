use anyhow::{Context, Result};

/// Check whether the working tree is clean (no uncommitted changes).
///
/// Returns `true` when there are no staged, unstaged, or untracked changes
/// (excluding ignored files).
#[allow(dead_code)]
pub fn working_tree_clean(repo: &git2::Repository) -> Result<bool> {
    let statuses = repo
        .statuses(Some(
            git2::StatusOptions::new()
                .include_untracked(true)
                .recurse_untracked_dirs(true),
        ))
        .context("failed to query working tree status")?;

    Ok(statuses.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn clean_after_commit_dirty_after_modification() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();

        // Configure author for commits
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();

        // Create initial commit so HEAD exists
        let file_path = dir.path().join("file.txt");
        fs::write(&file_path, "hello").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("file.txt")).unwrap();
        index.write().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .unwrap();

        // After commit the tree should be clean
        assert!(working_tree_clean(&repo).unwrap());

        // Modify a tracked file — tree should be dirty
        fs::write(&file_path, "world").unwrap();
        assert!(!working_tree_clean(&repo).unwrap());
    }
}
