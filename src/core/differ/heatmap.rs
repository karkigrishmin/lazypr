use crate::core::{DiffFile, FileCategory, Hunk, HunkClassification, LineKind, ReviewPriority};

pub fn category_weight(category: &FileCategory) -> f64 {
    match category {
        FileCategory::Lock | FileCategory::Generated | FileCategory::Snapshot => 0.0,
        FileCategory::Documentation => 0.2,
        FileCategory::Config => 0.3,
        FileCategory::Style => 0.4,
        FileCategory::Test => 0.5,
        FileCategory::TypeDefinition => 0.6,
        FileCategory::Unknown => 0.7,
        FileCategory::Source => 1.0,
    }
}

pub fn count_logic_lines(hunks: &[Hunk]) -> (usize, usize) {
    let mut new_logic = 0usize;
    let mut modified_logic = 0usize;

    for hunk in hunks {
        let non_blank_added = hunk
            .lines
            .iter()
            .filter(|l| l.kind == LineKind::Added && !l.content.trim().is_empty())
            .count();

        match &hunk.classification {
            HunkClassification::NewLogic => new_logic += non_blank_added,
            HunkClassification::ModifiedLogic => modified_logic += non_blank_added,
            _ => {}
        }
    }

    (new_logic, modified_logic)
}

pub fn score_file(file: &DiffFile) -> (f64, ReviewPriority) {
    let weight = category_weight(&file.category);
    if weight == 0.0 {
        return (0.0, ReviewPriority::Skip);
    }

    let (new_logic, modified_logic) = count_logic_lines(&file.hunks);
    let logic_score = (new_logic * 3 + modified_logic * 2) as f64;
    let risk = 1.0_f64;
    let priority_score = logic_score * weight * risk;

    let priority = if priority_score > 100.0 {
        ReviewPriority::Deep
    } else if priority_score > 20.0 {
        ReviewPriority::Scan
    } else if priority_score > 0.0 {
        ReviewPriority::Glance
    } else {
        ReviewPriority::Skip
    };

    (priority_score, priority)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{DiffLine, FileStats, FileStatus};

    fn make_file(category: FileCategory, hunks: Vec<Hunk>) -> DiffFile {
        DiffFile {
            path: "test.rs".into(),
            old_path: None,
            status: FileStatus::Modified,
            category,
            hunks,
            stats: FileStats::default(),
            priority: ReviewPriority::Glance,
            priority_score: 0.0,
            semantic_diff: None,
        }
    }

    fn make_hunk_with_classification(
        classification: HunkClassification,
        added: usize,
        removed: usize,
    ) -> Hunk {
        let mut lines = Vec::new();
        for _ in 0..removed {
            lines.push(DiffLine {
                kind: LineKind::Removed,
                content: "old code".into(),
                old_line_no: None,
                new_line_no: None,
            });
        }
        for _ in 0..added {
            lines.push(DiffLine {
                kind: LineKind::Added,
                content: "new code".into(),
                old_line_no: None,
                new_line_no: None,
            });
        }
        Hunk {
            old_start: 1,
            old_count: removed as u32,
            new_start: 1,
            new_count: added as u32,
            lines,
            classification,
        }
    }

    #[test]
    fn lock_files_have_zero_weight() {
        assert_eq!(category_weight(&FileCategory::Lock), 0.0);
        assert_eq!(category_weight(&FileCategory::Generated), 0.0);
        assert_eq!(category_weight(&FileCategory::Snapshot), 0.0);
    }

    #[test]
    fn source_files_have_full_weight() {
        assert_eq!(category_weight(&FileCategory::Source), 1.0);
    }

    #[test]
    fn category_weights_match_spec() {
        assert_eq!(category_weight(&FileCategory::Documentation), 0.2);
        assert_eq!(category_weight(&FileCategory::Config), 0.3);
        assert_eq!(category_weight(&FileCategory::Style), 0.4);
        assert_eq!(category_weight(&FileCategory::Test), 0.5);
        assert_eq!(category_weight(&FileCategory::TypeDefinition), 0.6);
        assert_eq!(category_weight(&FileCategory::Unknown), 0.7);
    }

    #[test]
    fn count_logic_lines_separates_new_from_modified() {
        let hunks = vec![
            make_hunk_with_classification(HunkClassification::NewLogic, 10, 0),
            make_hunk_with_classification(HunkClassification::ModifiedLogic, 5, 3),
        ];
        let (new_logic, modified_logic) = count_logic_lines(&hunks);
        assert_eq!(new_logic, 10);
        assert_eq!(modified_logic, 5);
    }

    #[test]
    fn import_only_hunks_contribute_zero_logic_lines() {
        let hunks = vec![make_hunk_with_classification(
            HunkClassification::ImportOnly,
            5,
            2,
        )];
        let (new_logic, modified_logic) = count_logic_lines(&hunks);
        assert_eq!(new_logic, 0);
        assert_eq!(modified_logic, 0);
    }

    #[test]
    fn blank_lines_are_excluded_from_logic_count() {
        let mut hunk = make_hunk_with_classification(HunkClassification::NewLogic, 3, 0);
        hunk.lines[0].content = "   ".to_string();
        let (new_logic, _) = count_logic_lines(&[hunk]);
        assert_eq!(new_logic, 2);
    }

    #[test]
    fn score_file_assigns_skip_for_lockfiles() {
        let file = make_file(
            FileCategory::Lock,
            vec![make_hunk_with_classification(
                HunkClassification::ModifiedLogic,
                100,
                50,
            )],
        );
        let (score, priority) = score_file(&file);
        assert_eq!(score, 0.0);
        assert_eq!(priority, ReviewPriority::Skip);
    }

    #[test]
    fn score_file_assigns_deep_for_large_source_changes() {
        let file = make_file(
            FileCategory::Source,
            vec![make_hunk_with_classification(
                HunkClassification::NewLogic,
                50,
                0,
            )],
        );
        let (score, priority) = score_file(&file);
        assert!(score > 100.0);
        assert_eq!(priority, ReviewPriority::Deep);
    }

    #[test]
    fn score_file_assigns_scan_for_moderate_changes() {
        let file = make_file(
            FileCategory::Source,
            vec![make_hunk_with_classification(
                HunkClassification::NewLogic,
                10,
                0,
            )],
        );
        let (_, priority) = score_file(&file);
        assert_eq!(priority, ReviewPriority::Scan);
    }

    #[test]
    fn score_file_assigns_glance_for_small_changes() {
        let file = make_file(
            FileCategory::Source,
            vec![make_hunk_with_classification(
                HunkClassification::NewLogic,
                2,
                0,
            )],
        );
        let (_, priority) = score_file(&file);
        assert_eq!(priority, ReviewPriority::Glance);
    }
}
