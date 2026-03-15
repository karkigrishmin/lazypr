use std::collections::HashMap;

use crate::core::analyzer::file_classifier::classify_file;
use crate::core::differ::classifier::classify_hunk;
use crate::core::differ::heatmap::{count_logic_lines, score_file};
use crate::core::differ::three_color::{detect_moves, MoveDetectionConfig};
use crate::core::{DiffResult, DiffSummary, FileCategory, FileStatus};
use crate::state::config::ReviewConfig;

/// Run the full analysis pipeline on a raw DiffResult, mutating it in place.
pub fn analyze(
    diff: &mut DiffResult,
    config: &ReviewConfig,
    provider: Option<&crate::core::git::Git2DiffProvider>,
) {
    // Step 1: Classify each file
    for file in diff.files.iter_mut() {
        file.category = classify_file(&file.path);
    }

    // Step 2: Classify each hunk
    for file in diff.files.iter_mut() {
        for hunk in file.hunks.iter_mut() {
            hunk.classification = classify_hunk(hunk);
        }
    }

    // Step 3: Run move detection
    let move_config = MoveDetectionConfig::from(config);
    let moved_lines = detect_moves(&mut diff.files, &move_config);

    // Step 4: Compute semantic diff for source files
    if let Some(provider) = provider {
        for file in diff.files.iter_mut() {
            if file.category != FileCategory::Source {
                continue;
            }
            if file.status == FileStatus::Deleted {
                continue;
            }

            let old_content = provider
                .file_at_ref(&diff.base_ref, &file.path)
                .ok()
                .flatten();
            let new_content = provider
                .file_at_ref(&diff.head_ref, &file.path)
                .ok()
                .flatten();

            if let (Some(old), Some(new)) = (old_content, new_content) {
                file.semantic_diff = Some(crate::core::differ::semantic::compute_semantic_diff(
                    &old, &new, &file.path,
                ));
            }
        }
    }

    // Step 5: Compute file churn if provider available
    let churn_map = if let Some(provider) = provider {
        let paths: Vec<String> = diff.files.iter().map(|f| f.path.clone()).collect();
        crate::core::git::log::compute_file_churn(provider.repo(), &paths, 30).unwrap_or_default()
    } else {
        HashMap::new()
    };

    // Step 6: Score each file with churn risk
    for file in diff.files.iter_mut() {
        let (new_logic, modified_logic) = count_logic_lines(&file.hunks);
        file.stats.logic_lines = new_logic + modified_logic;
        let risk = churn_map
            .get(&file.path)
            .map(|c| c.risk_multiplier)
            .unwrap_or(1.0);
        let (score, priority) = score_file(file, risk);
        file.priority_score = score;
        file.priority = priority;
    }

    // Step 7: Sort files by priority_score descending
    diff.files.sort_by(|a, b| {
        b.priority_score
            .partial_cmp(&a.priority_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Step 8: Rebuild summary from enriched files
    let total_files = diff.files.len();
    let mut files_by_priority = HashMap::new();
    let mut total_additions = 0usize;
    let mut total_deletions = 0usize;
    let mut logic_lines_added = 0usize;

    for file in diff.files.iter() {
        *files_by_priority.entry(file.priority.clone()).or_insert(0) += 1;
        total_additions += file.stats.additions;
        total_deletions += file.stats.deletions;
        logic_lines_added += file.stats.logic_lines;
    }

    let estimated_review_minutes = if total_files > 0 {
        ((logic_lines_added / 20) as u32).max(1)
    } else {
        0
    };

    diff.summary = DiffSummary {
        total_files,
        files_by_priority,
        total_additions,
        total_deletions,
        logic_lines_added,
        moved_lines,
        estimated_review_minutes,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::*;
    use crate::state::config::ReviewConfig;

    fn make_raw_diff() -> DiffResult {
        DiffResult {
            base_ref: "main".into(),
            head_ref: "HEAD".into(),
            files: vec![
                DiffFile {
                    path: "package-lock.json".into(),
                    old_path: None,
                    status: FileStatus::Modified,
                    category: FileCategory::Unknown,
                    hunks: vec![Hunk {
                        old_start: 1,
                        old_count: 5,
                        new_start: 1,
                        new_count: 10,
                        lines: (0..5)
                            .map(|_| DiffLine {
                                kind: LineKind::Added,
                                content: "  \"version\": \"1.2.3\",".into(),
                                old_line_no: None,
                                new_line_no: None,
                            })
                            .collect(),
                        classification: HunkClassification::ModifiedLogic,
                    }],
                    stats: FileStats {
                        additions: 5,
                        deletions: 0,
                        logic_lines: 0,
                    },
                    priority: ReviewPriority::Glance,
                    priority_score: 0.0,
                    semantic_diff: None,
                },
                DiffFile {
                    path: "src/main.rs".into(),
                    old_path: None,
                    status: FileStatus::Modified,
                    category: FileCategory::Unknown,
                    hunks: vec![Hunk {
                        old_start: 10,
                        old_count: 3,
                        new_start: 10,
                        new_count: 15,
                        lines: (0..12)
                            .map(|_| DiffLine {
                                kind: LineKind::Added,
                                content: "    let result = compute();".into(),
                                old_line_no: None,
                                new_line_no: None,
                            })
                            .collect(),
                        classification: HunkClassification::ModifiedLogic,
                    }],
                    stats: FileStats {
                        additions: 12,
                        deletions: 0,
                        logic_lines: 0,
                    },
                    priority: ReviewPriority::Glance,
                    priority_score: 0.0,
                    semantic_diff: None,
                },
            ],
            summary: DiffSummary::default(),
        }
    }

    #[test]
    fn pipeline_classifies_file_categories() {
        let mut diff = make_raw_diff();
        analyze(&mut diff, &ReviewConfig::default(), None);
        assert_eq!(
            diff.files
                .iter()
                .find(|f| f.path == "package-lock.json")
                .unwrap()
                .category,
            FileCategory::Lock
        );
        assert_eq!(
            diff.files
                .iter()
                .find(|f| f.path == "src/main.rs")
                .unwrap()
                .category,
            FileCategory::Source
        );
    }

    #[test]
    fn pipeline_sorts_source_before_lockfile() {
        let mut diff = make_raw_diff();
        analyze(&mut diff, &ReviewConfig::default(), None);
        assert_eq!(diff.files[0].path, "src/main.rs");
        assert_eq!(diff.files[1].path, "package-lock.json");
    }

    #[test]
    fn pipeline_assigns_skip_to_lockfiles() {
        let mut diff = make_raw_diff();
        analyze(&mut diff, &ReviewConfig::default(), None);
        let lock = diff
            .files
            .iter()
            .find(|f| f.path == "package-lock.json")
            .unwrap();
        assert_eq!(lock.priority, ReviewPriority::Skip);
        assert_eq!(lock.priority_score, 0.0);
    }

    #[test]
    fn pipeline_builds_summary() {
        let mut diff = make_raw_diff();
        analyze(&mut diff, &ReviewConfig::default(), None);
        assert_eq!(diff.summary.total_files, 2);
        assert!(diff.summary.total_additions > 0);
    }

    #[test]
    fn pipeline_detects_moved_code_between_files() {
        let moved_content: Vec<DiffLine> = (0..5)
            .map(|i| DiffLine {
                kind: LineKind::Removed,
                content: format!("    line_{i}();"),
                old_line_no: Some(i + 1),
                new_line_no: None,
            })
            .collect();
        let added_content: Vec<DiffLine> = (0..5)
            .map(|i| DiffLine {
                kind: LineKind::Added,
                content: format!("    line_{i}();"),
                old_line_no: None,
                new_line_no: Some(i + 1),
            })
            .collect();
        let mut diff = DiffResult {
            base_ref: "main".into(),
            head_ref: "HEAD".into(),
            files: vec![
                DiffFile {
                    path: "src/old.rs".into(),
                    old_path: None,
                    status: FileStatus::Modified,
                    category: FileCategory::Unknown,
                    hunks: vec![Hunk {
                        old_start: 1,
                        old_count: 5,
                        new_start: 1,
                        new_count: 0,
                        lines: moved_content,
                        classification: HunkClassification::ModifiedLogic,
                    }],
                    stats: FileStats {
                        additions: 0,
                        deletions: 5,
                        logic_lines: 0,
                    },
                    priority: ReviewPriority::Glance,
                    priority_score: 0.0,
                    semantic_diff: None,
                },
                DiffFile {
                    path: "src/new.rs".into(),
                    old_path: None,
                    status: FileStatus::Modified,
                    category: FileCategory::Unknown,
                    hunks: vec![Hunk {
                        old_start: 1,
                        old_count: 0,
                        new_start: 1,
                        new_count: 5,
                        lines: added_content,
                        classification: HunkClassification::ModifiedLogic,
                    }],
                    stats: FileStats {
                        additions: 5,
                        deletions: 0,
                        logic_lines: 0,
                    },
                    priority: ReviewPriority::Glance,
                    priority_score: 0.0,
                    semantic_diff: None,
                },
            ],
            summary: DiffSummary::default(),
        };
        analyze(&mut diff, &ReviewConfig::default(), None);
        assert!(diff.summary.moved_lines > 0, "should detect moved lines");
    }
}
