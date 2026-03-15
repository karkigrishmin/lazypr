use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::core::types::{
    DiffFile, DiffLine, DiffResult, DiffSummary, FileCategory, FileStats, FileStatus, Hunk,
    HunkClassification, LineKind, ReviewPriority,
};

use super::DiffProvider;

/// Git2-backed implementation of [`DiffProvider`].
///
/// Wraps a [`git2::Repository`] and computes diffs between two refs using
/// libgit2's tree-to-tree diff.
pub struct Git2DiffProvider {
    repo: git2::Repository,
}

impl Git2DiffProvider {
    /// Create a provider from an already-opened repository.
    #[allow(dead_code)]
    pub fn new(repo: git2::Repository) -> Self {
        Self { repo }
    }

    /// Open the repository that contains the current working directory.
    pub fn open() -> Result<Self> {
        let repo = git2::Repository::discover(".").context("failed to discover git repository")?;
        Ok(Self { repo })
    }

    /// Open the repository at the given path.
    #[allow(dead_code)]
    pub fn open_at(path: &std::path::Path) -> Result<Self> {
        let repo = git2::Repository::open(path).context("failed to open git repository at path")?;
        Ok(Self { repo })
    }

    /// Expose the underlying repository for callers that need branch utilities.
    pub fn repo(&self) -> &git2::Repository {
        &self.repo
    }
}

impl DiffProvider for Git2DiffProvider {
    fn diff(&self, base: &str, head: &str) -> Result<DiffResult> {
        let base_tree = resolve_tree(&self.repo, base)
            .with_context(|| format!("failed to resolve base ref '{base}'"))?;
        let head_tree = resolve_tree(&self.repo, head)
            .with_context(|| format!("failed to resolve head ref '{head}'"))?;

        let diff = self
            .repo
            .diff_tree_to_tree(Some(&base_tree), Some(&head_tree), None)
            .context("failed to compute tree-to-tree diff")?;

        build_diff_result(&diff, base, head)
    }
}

/// Resolve a ref string (branch name, tag, SHA, "HEAD", etc.) to a [`git2::Tree`].
fn resolve_tree<'r>(repo: &'r git2::Repository, refspec: &str) -> Result<git2::Tree<'r>> {
    let obj = repo
        .revparse_single(refspec)
        .with_context(|| format!("ref '{refspec}' not found"))?;
    let commit = obj
        .peel_to_commit()
        .with_context(|| format!("ref '{refspec}' does not point to a commit"))?;
    let tree = commit
        .tree()
        .with_context(|| format!("commit for '{refspec}' has no tree"))?;
    Ok(tree)
}

/// Convert a [`git2::Delta`] into our [`FileStatus`].
fn map_delta_status(delta: git2::Delta) -> FileStatus {
    match delta {
        git2::Delta::Added | git2::Delta::Untracked => FileStatus::Added,
        git2::Delta::Deleted => FileStatus::Deleted,
        git2::Delta::Renamed | git2::Delta::Copied => FileStatus::Renamed,
        _ => FileStatus::Modified,
    }
}

/// Build a complete [`DiffResult`] from a raw [`git2::Diff`].
fn build_diff_result(diff: &git2::Diff<'_>, base: &str, head: &str) -> Result<DiffResult> {
    let mut files: Vec<DiffFile> = Vec::new();

    // First pass: create a DiffFile entry for each delta
    let num_deltas = diff.deltas().len();
    for i in 0..num_deltas {
        let delta = diff.get_delta(i).expect("delta index in range");
        let new_file = delta.new_file();
        let old_file = delta.old_file();

        let path = new_file
            .path()
            .or_else(|| old_file.path())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let old_path = if delta.status() == git2::Delta::Renamed {
            old_file.path().map(|p| p.to_string_lossy().to_string())
        } else {
            None
        };

        let status = map_delta_status(delta.status());

        files.push(DiffFile {
            path,
            old_path,
            status,
            category: FileCategory::Unknown,
            hunks: Vec::new(),
            stats: FileStats::default(),
            priority: ReviewPriority::Glance,
            priority_score: 0.0,
        });
    }

    // Second pass: walk hunks and lines to populate each file
    let files_cell = std::cell::RefCell::new(&mut files);

    diff.foreach(
        &mut |_delta, _progress| true, // file_cb
        None,                          // binary_cb
        Some(&mut |delta, hunk| {
            let file_idx = find_delta_index(diff, &delta);
            let mut files_ref = files_cell.borrow_mut();
            if let Some(file) = files_ref.get_mut(file_idx) {
                file.hunks.push(Hunk {
                    old_start: hunk.old_start(),
                    old_count: hunk.old_lines(),
                    new_start: hunk.new_start(),
                    new_count: hunk.new_lines(),
                    lines: Vec::new(),
                    classification: HunkClassification::ModifiedLogic,
                });
            }
            true
        }),
        Some(&mut |delta, _hunk, line| {
            let file_idx = find_delta_index(diff, &delta);
            let mut files_ref = files_cell.borrow_mut();
            if let Some(file) = files_ref.get_mut(file_idx) {
                let (kind, is_add, is_del) = match line.origin() {
                    '+' => (LineKind::Added, true, false),
                    '-' => (LineKind::Removed, false, true),
                    _ => (LineKind::Context, false, false),
                };

                let content = String::from_utf8_lossy(line.content()).to_string();

                let diff_line = DiffLine {
                    kind,
                    content,
                    old_line_no: line.old_lineno(),
                    new_line_no: line.new_lineno(),
                };

                if let Some(hunk) = file.hunks.last_mut() {
                    hunk.lines.push(diff_line);
                }

                if is_add {
                    file.stats.additions += 1;
                }
                if is_del {
                    file.stats.deletions += 1;
                }
            }
            true
        }),
    )
    .context("failed to iterate over diff")?;

    // Build summary
    let summary = build_summary(&files);

    Ok(DiffResult {
        base_ref: base.to_string(),
        head_ref: head.to_string(),
        files,
        summary,
    })
}

/// Find the index of a delta within the diff by matching file paths.
fn find_delta_index(diff: &git2::Diff<'_>, delta: &git2::DiffDelta<'_>) -> usize {
    let target_path = delta.new_file().path().or_else(|| delta.old_file().path());

    for i in 0..diff.deltas().len() {
        let d = diff.get_delta(i).expect("delta index in range");
        let d_path = d.new_file().path().or_else(|| d.old_file().path());
        if d_path == target_path && d.status() == delta.status() {
            return i;
        }
    }
    0
}

/// Compute aggregate [`DiffSummary`] from the list of diff files.
fn build_summary(files: &[DiffFile]) -> DiffSummary {
    let mut files_by_priority: HashMap<ReviewPriority, usize> = HashMap::new();
    let mut total_additions: usize = 0;
    let mut total_deletions: usize = 0;
    let mut logic_lines_added: usize = 0;

    for f in files {
        *files_by_priority.entry(f.priority.clone()).or_default() += 1;
        total_additions += f.stats.additions;
        total_deletions += f.stats.deletions;
        logic_lines_added += f.stats.logic_lines;
    }

    // Simple heuristic: 1 minute per 20 changed lines, minimum 1
    let total_changed = total_additions + total_deletions;
    let estimated_review_minutes = ((total_changed as f64 / 20.0).ceil() as u32).max(1);

    DiffSummary {
        total_files: files.len(),
        files_by_priority,
        total_additions,
        total_deletions,
        logic_lines_added,
        moved_lines: 0, // Phase 0: no move detection
        estimated_review_minutes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::git::branch::{current_branch, detect_base_branch};
    use crate::core::git::status::working_tree_clean;
    use std::fs;
    use std::path::Path;

    /// Helper: create a temp repo with an initial commit on "main",
    /// then create and checkout a feature branch.
    fn setup_test_repo() -> (tempfile::TempDir, git2::Repository) {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();

        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();

        // Create initial file and commit on main
        let file_a = dir.path().join("existing.txt");
        fs::write(&file_a, "line one\nline two\nline three\n").unwrap();

        let initial = {
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("existing.txt")).unwrap();
            index.write().unwrap();
            let tree_oid = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_oid).unwrap();
            let sig = repo.signature().unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
                .unwrap()
        };

        // Rename default branch to "main"
        repo.reference("refs/heads/main", initial, true, "create main")
            .unwrap();
        repo.set_head("refs/heads/main").unwrap();
        if let Ok(mut master_ref) = repo.find_reference("refs/heads/master") {
            master_ref.delete().unwrap();
        }

        // Create and checkout feature branch
        {
            let commit_obj = repo.find_commit(initial).unwrap();
            repo.branch("feature/test", &commit_obj, false).unwrap();
        }
        repo.set_head("refs/heads/feature/test").unwrap();
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
            .unwrap();

        // Add a new file on the feature branch
        let file_b = dir.path().join("new_file.txt");
        fs::write(&file_b, "brand new content\n").unwrap();

        // Modify existing file
        fs::write(
            &file_a,
            "line one\nline two modified\nline three\nline four\n",
        )
        .unwrap();

        // Commit changes on feature branch
        {
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("existing.txt")).unwrap();
            index.add_path(Path::new("new_file.txt")).unwrap();
            index.write().unwrap();
            let tree_oid = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_oid).unwrap();
            let sig = repo.signature().unwrap();
            let parent = repo.find_commit(initial).unwrap();
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "feature changes",
                &tree,
                &[&parent],
            )
            .unwrap();
        }

        (dir, repo)
    }

    #[test]
    fn diff_detects_added_and_modified_files() {
        let (_dir, repo) = setup_test_repo();
        let provider = Git2DiffProvider::new(repo);

        let result = provider.diff("main", "feature/test").unwrap();

        assert_eq!(result.base_ref, "main");
        assert_eq!(result.head_ref, "feature/test");
        assert_eq!(result.files.len(), 2);

        // Sort by path for deterministic assertions
        let mut files = result.files.clone();
        files.sort_by(|a, b| a.path.cmp(&b.path));

        let existing = &files[0];
        assert_eq!(existing.path, "existing.txt");
        assert_eq!(existing.status, FileStatus::Modified);
        assert!(existing.stats.additions > 0 || existing.stats.deletions > 0);

        let new_file = &files[1];
        assert_eq!(new_file.path, "new_file.txt");
        assert_eq!(new_file.status, FileStatus::Added);
        assert!(new_file.stats.additions > 0);
    }

    #[test]
    fn diff_summary_has_correct_counts() {
        let (_dir, repo) = setup_test_repo();
        let provider = Git2DiffProvider::new(repo);

        let result = provider.diff("main", "feature/test").unwrap();

        assert_eq!(result.summary.total_files, 2);
        assert!(result.summary.total_additions > 0);
        assert!(result.summary.estimated_review_minutes >= 1);
    }

    #[test]
    fn diff_hunks_contain_lines() {
        let (_dir, repo) = setup_test_repo();
        let provider = Git2DiffProvider::new(repo);

        let result = provider.diff("main", "feature/test").unwrap();

        for file in &result.files {
            assert!(
                !file.hunks.is_empty(),
                "file {} should have at least one hunk",
                file.path
            );
            for hunk in &file.hunks {
                assert!(
                    !hunk.lines.is_empty(),
                    "hunk in {} should have lines",
                    file.path
                );
            }
        }
    }

    #[test]
    fn diff_with_invalid_base_returns_error() {
        let (_dir, repo) = setup_test_repo();
        let provider = Git2DiffProvider::new(repo);

        let result = provider.diff("nonexistent-branch", "feature/test");
        assert!(result.is_err());
        let err_msg = format!("{:#}", result.unwrap_err());
        assert!(
            err_msg.contains("nonexistent-branch"),
            "error should mention the bad ref: {err_msg}"
        );
    }

    #[test]
    fn phase0_defaults_for_category_and_priority() {
        let (_dir, repo) = setup_test_repo();
        let provider = Git2DiffProvider::new(repo);

        let result = provider.diff("main", "feature/test").unwrap();

        for file in &result.files {
            assert_eq!(file.category, FileCategory::Unknown);
            assert_eq!(file.priority, ReviewPriority::Glance);
            for hunk in &file.hunks {
                assert_eq!(hunk.classification, HunkClassification::ModifiedLogic);
            }
        }
    }

    #[test]
    fn detect_base_branch_finds_main_in_test_repo() {
        let (_dir, repo) = setup_test_repo();
        let base = detect_base_branch(&repo).unwrap();
        assert_eq!(base, "main");
    }

    #[test]
    fn current_branch_returns_feature_branch() {
        let (_dir, repo) = setup_test_repo();
        let name = current_branch(&repo).unwrap();
        assert_eq!(name, "feature/test");
    }

    #[test]
    fn working_tree_clean_after_commit() {
        let (_dir, repo) = setup_test_repo();
        assert!(working_tree_clean(&repo).unwrap());
    }

    #[test]
    fn working_tree_dirty_after_modification() {
        let (dir, repo) = setup_test_repo();
        let file = dir.path().join("existing.txt");
        fs::write(&file, "dirty change").unwrap();
        assert!(!working_tree_clean(&repo).unwrap());
    }

    #[test]
    fn diff_deleted_file() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();

        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();

        // Initial commit with a file
        let initial = {
            let file_path = dir.path().join("to_delete.txt");
            fs::write(&file_path, "content\n").unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("to_delete.txt")).unwrap();
            index.write().unwrap();
            let tree_oid = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_oid).unwrap();
            let sig = repo.signature().unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
                .unwrap()
        };

        repo.reference("refs/heads/main", initial, true, "main")
            .unwrap();
        repo.set_head("refs/heads/main").unwrap();
        if let Ok(mut master_ref) = repo.find_reference("refs/heads/master") {
            master_ref.delete().unwrap();
        }

        // Feature branch: remove the file
        {
            let parent = repo.find_commit(initial).unwrap();
            repo.branch("feature/del", &parent, false).unwrap();
        }
        repo.set_head("refs/heads/feature/del").unwrap();

        {
            let mut index = repo.index().unwrap();
            index.remove_path(Path::new("to_delete.txt")).unwrap();
            index.write().unwrap();
            let tree_oid = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_oid).unwrap();
            let sig = repo.signature().unwrap();
            let parent = repo.find_commit(initial).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "delete file", &tree, &[&parent])
                .unwrap();
        }

        let provider = Git2DiffProvider::new(repo);
        let result = provider.diff("main", "feature/del").unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].status, FileStatus::Deleted);
        assert_eq!(result.files[0].path, "to_delete.txt");
        assert!(result.files[0].stats.deletions > 0);
    }
}
