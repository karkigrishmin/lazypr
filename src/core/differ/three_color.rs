use std::collections::HashMap;

use xxhash_rust::xxh3::xxh3_64;

use crate::core::{DiffFile, HunkClassification, LineKind};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

pub struct MoveDetectionConfig {
    pub min_lines: usize,
    pub similarity_threshold: f64,
}

impl Default for MoveDetectionConfig {
    fn default() -> Self {
        Self {
            min_lines: 3,
            similarity_threshold: 0.85,
        }
    }
}

impl From<&crate::state::config::ReviewConfig> for MoveDetectionConfig {
    fn from(config: &crate::state::config::ReviewConfig) -> Self {
        Self {
            min_lines: config.move_detection_min_lines,
            similarity_threshold: config.move_similarity_threshold,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helper
// ---------------------------------------------------------------------------

struct CodeBlock {
    file_idx: usize,
    hunk_idx: usize,
    line_start: usize, // index into hunk.lines (inclusive)
    line_end: usize,   // exclusive
    normalized: String,
    hash: u64,
}

fn normalize_block(lines: &[&str]) -> String {
    lines
        .iter()
        .map(|l| l.trim_start())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn collect_blocks(files: &[DiffFile], min_lines: usize, kind: LineKind) -> Vec<CodeBlock> {
    let mut blocks = Vec::new();

    for (file_idx, file) in files.iter().enumerate() {
        for (hunk_idx, hunk) in file.hunks.iter().enumerate() {
            let mut run_start: Option<usize> = None;

            for (line_idx, line) in hunk.lines.iter().enumerate() {
                if line.kind == kind {
                    if run_start.is_none() {
                        run_start = Some(line_idx);
                    }
                } else {
                    // End of run — flush if long enough
                    if let Some(start) = run_start.take() {
                        let end = line_idx;
                        if end - start >= min_lines {
                            let raw: Vec<&str> = hunk.lines[start..end]
                                .iter()
                                .map(|l| l.content.as_str())
                                .collect();
                            let normalized = normalize_block(&raw);
                            let hash = xxh3_64(normalized.as_bytes());
                            blocks.push(CodeBlock {
                                file_idx,
                                hunk_idx,
                                line_start: start,
                                line_end: end,
                                normalized,
                                hash,
                            });
                        }
                    }
                }
            }

            // Flush trailing run
            if let Some(start) = run_start.take() {
                let end = hunk.lines.len();
                if end - start >= min_lines {
                    let raw: Vec<&str> = hunk.lines[start..end]
                        .iter()
                        .map(|l| l.content.as_str())
                        .collect();
                    let normalized = normalize_block(&raw);
                    let hash = xxh3_64(normalized.as_bytes());
                    blocks.push(CodeBlock {
                        file_idx,
                        hunk_idx,
                        line_start: start,
                        line_end: end,
                        normalized,
                        hash,
                    });
                }
            }
        }
    }

    blocks
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Run move detection across all files, mutating DiffLine.kind in place.
/// Returns the total number of moved lines detected.
pub fn detect_moves(files: &mut [DiffFile], config: &MoveDetectionConfig) -> usize {
    // Collect blocks (immutable borrow, so we work with indices + content)
    let removed_blocks = collect_blocks(files, config.min_lines, LineKind::Removed);
    let added_blocks = collect_blocks(files, config.min_lines, LineKind::Added);

    // Build hash maps: hash -> Vec<block_index>
    let mut removed_by_hash: HashMap<u64, Vec<usize>> = HashMap::new();
    for (idx, block) in removed_blocks.iter().enumerate() {
        removed_by_hash.entry(block.hash).or_default().push(idx);
    }
    let mut added_by_hash: HashMap<u64, Vec<usize>> = HashMap::new();
    for (idx, block) in added_blocks.iter().enumerate() {
        added_by_hash.entry(block.hash).or_default().push(idx);
    }

    // Track which blocks are still unmatched
    let mut unmatched_removed: Vec<bool> = vec![true; removed_blocks.len()];
    let mut unmatched_added: Vec<bool> = vec![true; added_blocks.len()];

    // We'll accumulate mutations as (file_idx, hunk_idx, line_start, line_end, new_kind)
    let mut mutations: Vec<(usize, usize, usize, usize, LineKind)> = Vec::new();

    // -----------------------------------------------------------------------
    // Exact match pass
    // -----------------------------------------------------------------------
    for hash in removed_by_hash.keys() {
        if let Some(added_indices) = added_by_hash.get(hash) {
            let removed_indices = removed_by_hash.get(hash).unwrap();

            // Collect pairs first to avoid borrow conflicts
            let pairs: Vec<(usize, usize)> = removed_indices
                .iter()
                .filter(|&&ri| unmatched_removed[ri])
                .zip(added_indices.iter().filter(|&&ai| unmatched_added[ai]))
                .map(|(&ri, &ai)| (ri, ai))
                .collect();

            for (ri, ai) in pairs {
                let rb = &removed_blocks[ri];
                mutations.push((
                    rb.file_idx,
                    rb.hunk_idx,
                    rb.line_start,
                    rb.line_end,
                    LineKind::Moved,
                ));
                let ab = &added_blocks[ai];
                mutations.push((
                    ab.file_idx,
                    ab.hunk_idx,
                    ab.line_start,
                    ab.line_end,
                    LineKind::Moved,
                ));
                unmatched_removed[ri] = false;
                unmatched_added[ai] = false;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Fuzzy match pass (only for blocks with > 10 lines)
    // -----------------------------------------------------------------------
    let unmatched_r_indices: Vec<usize> = (0..removed_blocks.len())
        .filter(|&i| {
            unmatched_removed[i] && (removed_blocks[i].line_end - removed_blocks[i].line_start) > 10
        })
        .collect();

    let unmatched_a_indices: Vec<usize> = (0..added_blocks.len())
        .filter(|&i| {
            unmatched_added[i] && (added_blocks[i].line_end - added_blocks[i].line_start) > 10
        })
        .collect();

    // For each unmatched removed block, find the best matching added block
    let mut used_added: Vec<bool> = vec![false; added_blocks.len()];

    for &ri in &unmatched_r_indices {
        let rb = &removed_blocks[ri];
        let mut best_ai: Option<usize> = None;
        let mut best_ratio = config.similarity_threshold as f32;

        for &ai in &unmatched_a_indices {
            if used_added[ai] {
                continue;
            }
            let ab = &added_blocks[ai];
            let ratio = similar::TextDiff::from_chars(&rb.normalized, &ab.normalized).ratio();
            if ratio >= best_ratio {
                best_ratio = ratio;
                best_ai = Some(ai);
            }
        }

        if let Some(ai) = best_ai {
            used_added[ai] = true;
            let ab = &added_blocks[ai];
            mutations.push((
                rb.file_idx,
                rb.hunk_idx,
                rb.line_start,
                rb.line_end,
                LineKind::MovedEdited,
            ));
            mutations.push((
                ab.file_idx,
                ab.hunk_idx,
                ab.line_start,
                ab.line_end,
                LineKind::MovedEdited,
            ));
            unmatched_removed[ri] = false;
            unmatched_added[ai] = false;
        }
    }

    // -----------------------------------------------------------------------
    // Apply mutations
    // -----------------------------------------------------------------------
    let mut total = 0usize;
    for (file_idx, hunk_idx, line_start, line_end, new_kind) in &mutations {
        let hunk = &mut files[*file_idx].hunks[*hunk_idx];
        for line in &mut hunk.lines[*line_start..*line_end] {
            line.kind = new_kind.clone();
            total += 1;
        }
    }

    // -----------------------------------------------------------------------
    // Update hunk classifications
    // -----------------------------------------------------------------------
    for file in files.iter_mut() {
        for hunk in file.hunks.iter_mut() {
            // Check if ALL non-context lines are Moved or MovedEdited
            let non_ctx: Vec<&LineKind> = hunk
                .lines
                .iter()
                .filter(|l| l.kind != LineKind::Context)
                .map(|l| &l.kind)
                .collect();

            if non_ctx.is_empty() {
                continue;
            }

            let all_moved = non_ctx.iter().all(|k| **k == LineKind::Moved);
            let all_moved_or_edited = non_ctx
                .iter()
                .all(|k| **k == LineKind::Moved || **k == LineKind::MovedEdited);

            if all_moved {
                hunk.classification = HunkClassification::Moved {
                    from: String::new(),
                    similarity: 1.0,
                };
            } else if all_moved_or_edited {
                hunk.classification = HunkClassification::MovedWithEdits {
                    from: String::new(),
                    similarity: 0.0,
                };
            }
        }
    }

    total
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        DiffFile, DiffLine, FileCategory, FileStats, FileStatus, Hunk, HunkClassification,
        LineKind, ReviewPriority,
    };

    fn make_file(path: &str, hunks: Vec<Hunk>) -> DiffFile {
        DiffFile {
            path: path.to_string(),
            old_path: None,
            status: FileStatus::Modified,
            category: FileCategory::Source,
            hunks,
            stats: FileStats::default(),
            priority: ReviewPriority::Glance,
            priority_score: 0.0,
            semantic_diff: None,
        }
    }

    fn make_hunk(lines: Vec<(LineKind, &str)>) -> Hunk {
        Hunk {
            old_start: 1,
            old_count: 0,
            new_start: 1,
            new_count: 0,
            lines: lines
                .into_iter()
                .map(|(kind, content)| DiffLine {
                    kind,
                    content: content.to_string(),
                    old_line_no: None,
                    new_line_no: None,
                })
                .collect(),
            classification: HunkClassification::ModifiedLogic,
        }
    }

    #[test]
    fn exact_move_between_files_is_detected() {
        let removed_lines = vec![
            (LineKind::Removed, "fn helper() {"),
            (LineKind::Removed, "    do_thing();"),
            (LineKind::Removed, "    do_other();"),
        ];
        let added_lines = vec![
            (LineKind::Added, "fn helper() {"),
            (LineKind::Added, "    do_thing();"),
            (LineKind::Added, "    do_other();"),
        ];
        let mut files = vec![
            make_file("old.rs", vec![make_hunk(removed_lines)]),
            make_file("new.rs", vec![make_hunk(added_lines)]),
        ];
        let config = MoveDetectionConfig::default();
        let moved = detect_moves(&mut files, &config);
        assert!(moved > 0, "should detect moved lines");
        // Added lines should now be Moved
        assert!(files[1].hunks[0]
            .lines
            .iter()
            .all(|l| l.kind == LineKind::Moved));
    }

    #[test]
    fn whitespace_normalization_allows_reindented_matches() {
        let removed_lines = vec![
            (LineKind::Removed, "    fn helper() {"),
            (LineKind::Removed, "        do_thing();"),
            (LineKind::Removed, "    }"),
        ];
        let added_lines = vec![
            (LineKind::Added, "fn helper() {"),
            (LineKind::Added, "    do_thing();"),
            (LineKind::Added, "}"),
        ];
        let mut files = vec![
            make_file("old.rs", vec![make_hunk(removed_lines)]),
            make_file("new.rs", vec![make_hunk(added_lines)]),
        ];
        let config = MoveDetectionConfig::default();
        let moved = detect_moves(&mut files, &config);
        assert!(moved > 0, "re-indented code should be detected as moved");
    }

    #[test]
    fn blocks_shorter_than_min_lines_are_ignored() {
        let removed = vec![
            (LineKind::Removed, "short();"),
            (LineKind::Removed, "block();"),
        ];
        let added = vec![(LineKind::Added, "short();"), (LineKind::Added, "block();")];
        let mut files = vec![
            make_file("a.rs", vec![make_hunk(removed)]),
            make_file("b.rs", vec![make_hunk(added)]),
        ];
        let config = MoveDetectionConfig {
            min_lines: 3,
            similarity_threshold: 0.85,
        };
        let moved = detect_moves(&mut files, &config);
        assert_eq!(moved, 0, "2-line blocks should be ignored with min_lines=3");
    }

    #[test]
    fn fuzzy_move_with_edits_above_threshold_is_detected() {
        // 12 lines, mostly the same, a couple edited
        let mut removed_lines: Vec<(LineKind, &str)> = Vec::new();
        let mut added_lines: Vec<(LineKind, &str)> = Vec::new();
        for _ in 0..10 {
            removed_lines.push((LineKind::Removed, "    unchanged_line();"));
            added_lines.push((LineKind::Added, "    unchanged_line();"));
        }
        removed_lines.push((LineKind::Removed, "    old_specific();"));
        added_lines.push((LineKind::Added, "    new_specific();"));
        removed_lines.push((LineKind::Removed, "    another_old();"));
        added_lines.push((LineKind::Added, "    another_new();"));

        let mut files = vec![
            make_file("a.rs", vec![make_hunk(removed_lines)]),
            make_file("b.rs", vec![make_hunk(added_lines)]),
        ];
        let config = MoveDetectionConfig::default();
        let moved = detect_moves(&mut files, &config);
        assert!(
            moved > 0,
            "fuzzy match should be detected for similar blocks"
        );
        assert!(files[1].hunks[0]
            .lines
            .iter()
            .any(|l| l.kind == LineKind::MovedEdited));
    }

    #[test]
    fn fuzzy_move_below_threshold_is_not_matched() {
        // Very different blocks — should NOT match
        let mut removed_lines: Vec<(LineKind, &str)> = Vec::new();
        let mut added_lines: Vec<(LineKind, &str)> = Vec::new();
        for i in 0..12 {
            if i % 2 == 0 {
                removed_lines.push((LineKind::Removed, "    completely_different_old();"));
                added_lines.push((LineKind::Added, "    totally_new_code_here();"));
            } else {
                removed_lines.push((LineKind::Removed, "    another_old_thing();"));
                added_lines.push((LineKind::Added, "    another_new_thing();"));
            }
        }
        let mut files = vec![
            make_file("a.rs", vec![make_hunk(removed_lines)]),
            make_file("b.rs", vec![make_hunk(added_lines)]),
        ];
        let config = MoveDetectionConfig::default();
        let moved = detect_moves(&mut files, &config);
        assert_eq!(moved, 0, "very different blocks should not match");
    }

    #[test]
    fn move_within_same_file_is_detected() {
        let hunk1 = make_hunk(vec![
            (LineKind::Removed, "fn moved_fn() {"),
            (LineKind::Removed, "    body();"),
            (LineKind::Removed, "    more();"),
        ]);
        let hunk2 = make_hunk(vec![
            (LineKind::Added, "fn moved_fn() {"),
            (LineKind::Added, "    body();"),
            (LineKind::Added, "    more();"),
        ]);
        let mut files = vec![make_file("same.rs", vec![hunk1, hunk2])];
        let config = MoveDetectionConfig::default();
        let moved = detect_moves(&mut files, &config);
        assert!(moved > 0, "within-file moves should be detected");
    }

    #[test]
    fn moved_lines_get_correct_line_kind() {
        let removed = vec![
            (LineKind::Removed, "fn foo() {"),
            (LineKind::Removed, "    bar();"),
            (LineKind::Removed, "    baz();"),
        ];
        let added = vec![
            (LineKind::Added, "fn foo() {"),
            (LineKind::Added, "    bar();"),
            (LineKind::Added, "    baz();"),
        ];
        let mut files = vec![
            make_file("a.rs", vec![make_hunk(removed)]),
            make_file("b.rs", vec![make_hunk(added)]),
        ];
        let config = MoveDetectionConfig::default();
        detect_moves(&mut files, &config);
        // Removed lines should become Moved
        for line in &files[0].hunks[0].lines {
            assert_eq!(line.kind, LineKind::Moved);
        }
        // Added lines should become Moved
        for line in &files[1].hunks[0].lines {
            assert_eq!(line.kind, LineKind::Moved);
        }
    }
}
