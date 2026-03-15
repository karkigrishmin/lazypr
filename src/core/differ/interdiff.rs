#![allow(dead_code)]

use std::collections::HashMap;

use xxhash_rust::xxh3::xxh3_64;

use crate::core::{DiffFile, DiffResult};

/// Result of comparing two review-round diffs.
#[derive(Debug, Clone)]
pub struct InterDiffResult {
    /// Files present in the new diff but not the old.
    pub new_files: Vec<String>,
    /// Files present in both but with different content.
    pub modified_files: Vec<String>,
    /// Files present in the old diff but not the new.
    pub removed_files: Vec<String>,
    /// Files present in both with identical content.
    pub unchanged_files: Vec<String>,
}

/// Concatenate all hunk line content for a file and hash with xxh3.
fn hash_file_content(file: &DiffFile) -> u64 {
    let mut combined = String::new();
    for hunk in &file.hunks {
        for line in &hunk.lines {
            combined.push_str(&line.content);
            combined.push('\n');
        }
    }
    xxh3_64(combined.as_bytes())
}

/// Compare two `DiffResult` values and report which files are new, modified,
/// removed, or unchanged between review rounds.
pub fn compute_interdiff(old: &DiffResult, new: &DiffResult) -> InterDiffResult {
    let old_map: HashMap<&str, u64> = old
        .files
        .iter()
        .map(|f| (f.path.as_str(), hash_file_content(f)))
        .collect();

    let new_map: HashMap<&str, u64> = new
        .files
        .iter()
        .map(|f| (f.path.as_str(), hash_file_content(f)))
        .collect();

    let mut new_files = Vec::new();
    let mut modified_files = Vec::new();
    let mut unchanged_files = Vec::new();
    let mut removed_files = Vec::new();

    for (path, new_hash) in &new_map {
        match old_map.get(path) {
            None => new_files.push(path.to_string()),
            Some(old_hash) if old_hash == new_hash => unchanged_files.push(path.to_string()),
            Some(_) => modified_files.push(path.to_string()),
        }
    }

    for path in old_map.keys() {
        if !new_map.contains_key(path) {
            removed_files.push(path.to_string());
        }
    }

    // Sort for deterministic output.
    new_files.sort();
    modified_files.sort();
    removed_files.sort();
    unchanged_files.sort();

    InterDiffResult {
        new_files,
        modified_files,
        removed_files,
        unchanged_files,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        DiffFile, DiffLine, DiffResult, DiffSummary, FileCategory, FileStats, FileStatus, Hunk,
        HunkClassification, LineKind, ReviewPriority,
    };

    fn make_file(path: &str, content_line: &str) -> DiffFile {
        DiffFile {
            path: path.into(),
            old_path: None,
            status: FileStatus::Modified,
            category: FileCategory::Source,
            hunks: vec![Hunk {
                old_start: 1,
                old_count: 1,
                new_start: 1,
                new_count: 1,
                lines: vec![DiffLine {
                    kind: LineKind::Added,
                    content: content_line.into(),
                    old_line_no: None,
                    new_line_no: Some(1),
                }],
                classification: HunkClassification::ModifiedLogic,
            }],
            stats: FileStats {
                additions: 1,
                deletions: 0,
                logic_lines: 1,
            },
            priority: ReviewPriority::Scan,
            priority_score: 10.0,
        }
    }

    fn make_diff(files: Vec<DiffFile>) -> DiffResult {
        DiffResult {
            base_ref: "main".into(),
            head_ref: "HEAD".into(),
            files,
            summary: DiffSummary::default(),
        }
    }

    #[test]
    fn identical_diffs_all_unchanged() {
        let old = make_diff(vec![make_file("a.rs", "line1")]);
        let new = make_diff(vec![make_file("a.rs", "line1")]);
        let result = compute_interdiff(&old, &new);
        assert!(result.new_files.is_empty());
        assert!(result.modified_files.is_empty());
        assert!(result.removed_files.is_empty());
        assert_eq!(result.unchanged_files, vec!["a.rs"]);
    }

    #[test]
    fn new_file_detected() {
        let old = make_diff(vec![]);
        let new = make_diff(vec![make_file("a.rs", "line1")]);
        let result = compute_interdiff(&old, &new);
        assert_eq!(result.new_files, vec!["a.rs"]);
    }

    #[test]
    fn removed_file_detected() {
        let old = make_diff(vec![make_file("a.rs", "line1")]);
        let new = make_diff(vec![]);
        let result = compute_interdiff(&old, &new);
        assert_eq!(result.removed_files, vec!["a.rs"]);
    }

    #[test]
    fn modified_file_detected() {
        let old = make_diff(vec![make_file("a.rs", "line1")]);
        let new = make_diff(vec![make_file("a.rs", "line2_changed")]);
        let result = compute_interdiff(&old, &new);
        assert_eq!(result.modified_files, vec!["a.rs"]);
    }

    #[test]
    fn empty_diffs_produce_empty_result() {
        let result = compute_interdiff(&make_diff(vec![]), &make_diff(vec![]));
        assert!(result.new_files.is_empty());
        assert!(result.modified_files.is_empty());
        assert!(result.removed_files.is_empty());
        assert!(result.unchanged_files.is_empty());
    }

    #[test]
    fn mixed_scenario() {
        let old = make_diff(vec![
            make_file("kept.rs", "same"),
            make_file("changed.rs", "old_content"),
            make_file("gone.rs", "deleted"),
        ]);
        let new = make_diff(vec![
            make_file("kept.rs", "same"),
            make_file("changed.rs", "new_content"),
            make_file("added.rs", "brand_new"),
        ]);
        let result = compute_interdiff(&old, &new);
        assert_eq!(result.unchanged_files, vec!["kept.rs"]);
        assert_eq!(result.modified_files, vec!["changed.rs"]);
        assert_eq!(result.removed_files, vec!["gone.rs"]);
        assert_eq!(result.new_files, vec!["added.rs"]);
    }
}
