#![allow(dead_code)]

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Diff types
// ---------------------------------------------------------------------------

/// Status of a file in a diff (added, modified, deleted, or renamed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileStatus {
    /// Newly added file.
    Added,
    /// Modified existing file.
    Modified,
    /// Deleted file.
    Deleted,
    /// Renamed file (may also include edits).
    Renamed,
}

/// Broad category for a file based on its path and extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileCategory {
    /// Application source code.
    Source,
    /// Test files.
    Test,
    /// Type definition / declaration files (e.g. `.d.ts`).
    TypeDefinition,
    /// Configuration files (e.g. `tsconfig.json`, `.eslintrc`).
    Config,
    /// Stylesheets (CSS, SCSS, etc.).
    Style,
    /// Lock files (`Cargo.lock`, `package-lock.json`, etc.).
    Lock,
    /// Auto-generated code.
    Generated,
    /// Test / UI snapshots.
    Snapshot,
    /// Markdown, RST, and other documentation.
    Documentation,
    /// Anything that does not match another category.
    Unknown,
}

/// Review priority level assigned to a changed file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReviewPriority {
    /// New logic, high complexity -- requires deep review.
    Deep,
    /// Moderate changes -- scan through.
    Scan,
    /// Minor changes -- glance is enough.
    Glance,
    /// Lockfiles, generated, snapshots -- safe to skip.
    Skip,
}

/// Line-level addition / deletion / context statistics for a file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileStats {
    /// Number of added lines.
    pub additions: usize,
    /// Number of deleted lines.
    pub deletions: usize,
    /// Number of logic (non-blank, non-comment) lines added.
    pub logic_lines: usize,
}

/// Classification of a single hunk within a diff.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HunkClassification {
    /// Brand-new logic introduced.
    NewLogic,
    /// Existing logic modified.
    ModifiedLogic,
    /// Only import / use statements changed.
    ImportOnly,
    /// Only whitespace / formatting changed.
    WhitespaceOnly,
    /// Code moved from another location with no edits.
    Moved {
        /// Original file or location the code was moved from.
        from: String,
        /// Similarity score (0.0 .. 1.0).
        similarity: f64,
    },
    /// Code moved from another location with additional edits.
    MovedWithEdits {
        /// Original file or location the code was moved from.
        from: String,
        /// Similarity score (0.0 .. 1.0).
        similarity: f64,
    },
}

/// The kind of a single line in a diff hunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineKind {
    /// Unchanged context line.
    Context,
    /// Newly added line.
    Added,
    /// Removed line.
    Removed,
    /// Line detected as moved (unchanged content).
    Moved,
    /// Line detected as moved with edits.
    MovedEdited,
}

/// A single line in a diff hunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    /// What kind of diff line this is.
    pub kind: LineKind,
    /// The textual content of the line.
    pub content: String,
    /// Line number in the old (base) file, if applicable.
    pub old_line_no: Option<u32>,
    /// Line number in the new (head) file, if applicable.
    pub new_line_no: Option<u32>,
}

/// A contiguous hunk inside a diff for a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hunk {
    /// Starting line in the old file.
    pub old_start: u32,
    /// Number of lines from the old file.
    pub old_count: u32,
    /// Starting line in the new file.
    pub new_start: u32,
    /// Number of lines from the new file.
    pub new_count: u32,
    /// Individual diff lines within this hunk.
    pub lines: Vec<DiffLine>,
    /// Semantic classification of the hunk.
    pub classification: HunkClassification,
}

/// A single changed file in a diff, with parsed hunks and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffFile {
    /// Path of the file (new path if renamed).
    pub path: String,
    /// Previous path if the file was renamed.
    pub old_path: Option<String>,
    /// Whether the file was added, modified, deleted, or renamed.
    pub status: FileStatus,
    /// Broad category of the file.
    pub category: FileCategory,
    /// Diff hunks for this file.
    pub hunks: Vec<Hunk>,
    /// Line-level statistics.
    pub stats: FileStats,
    /// Assigned review priority.
    pub priority: ReviewPriority,
    /// Numeric priority score (0.0 -- 100.0+).
    pub priority_score: f64,
}

/// Aggregate summary of an entire diff.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiffSummary {
    /// Total number of changed files.
    pub total_files: usize,
    /// Count of files at each review-priority level.
    pub files_by_priority: HashMap<ReviewPriority, usize>,
    /// Total added lines across all files.
    pub total_additions: usize,
    /// Total deleted lines across all files.
    pub total_deletions: usize,
    /// Total logic lines added across all files.
    pub logic_lines_added: usize,
    /// Total detected moved lines.
    pub moved_lines: usize,
    /// Estimated review time in minutes.
    pub estimated_review_minutes: u32,
}

/// The complete result of diffing two refs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiffResult {
    /// The base (merge-base) ref.
    pub base_ref: String,
    /// The head (current) ref.
    pub head_ref: String,
    /// Changed files with hunks.
    pub files: Vec<DiffFile>,
    /// Aggregate summary.
    pub summary: DiffSummary,
}

// ---------------------------------------------------------------------------
// Parser types
// ---------------------------------------------------------------------------

/// Programming language detected for a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    /// TypeScript (.ts, .tsx).
    TypeScript,
    /// JavaScript (.js, .jsx, .mjs, .cjs).
    JavaScript,
    /// Python (.py).
    Python,
    /// Rust (.rs).
    Rust,
    /// Unrecognised or unsupported language.
    Unknown,
}

/// A single import statement parsed from a source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Import {
    /// Module specifier or path (e.g. `"react"`, `"./utils"`).
    pub source: String,
    /// Named imports (e.g. `useState`, `useEffect`).
    pub names: Vec<String>,
    /// Default import name, if any.
    pub default: Option<String>,
    /// Resolved absolute path on disk, if available.
    pub resolved_path: Option<String>,
}

/// The kind of an export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportKind {
    /// A named export.
    Named,
    /// A default export.
    Default,
    /// A re-export from another module.
    ReExport {
        /// The source module being re-exported.
        source: String,
    },
}

/// A single export parsed from a source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Export {
    /// The exported name.
    pub name: String,
    /// What kind of export this is.
    pub kind: ExportKind,
}

/// A function / method signature extracted from source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionSignature {
    /// Function or method name.
    pub name: String,
    /// Parameter names (types omitted for brevity).
    pub params: Vec<String>,
    /// Return type annotation, if present.
    pub return_type: Option<String>,
    /// Line number where the function is defined.
    pub line: u32,
}

/// Parsed structural information for a single source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedFile {
    /// File path relative to the repository root.
    pub path: String,
    /// Detected programming language.
    pub language: Language,
    /// Import statements found in the file.
    pub imports: Vec<Import>,
    /// Export statements found in the file.
    pub exports: Vec<Export>,
    /// Function / method signatures found in the file.
    pub functions: Vec<FunctionSignature>,
}

// ---------------------------------------------------------------------------
// Splitter types
// ---------------------------------------------------------------------------

/// Aggregate statistics for a split group.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupStats {
    /// Number of files in this group.
    pub total_files: usize,
    /// Total added lines in this group.
    pub total_additions: usize,
    /// Total deleted lines in this group.
    pub total_deletions: usize,
    /// Total logic lines in this group.
    pub logic_lines: usize,
}

/// A group of related files that should be reviewed together.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitGroup {
    /// Zero-based index of this group.
    pub index: usize,
    /// Human-readable group name / label.
    pub name: String,
    /// Paths of files belonging to this group.
    pub files: Vec<String>,
    /// Indices of groups that this group depends on.
    pub depends_on: Vec<usize>,
    /// Aggregate statistics.
    pub stats: GroupStats,
}

/// A complete plan for splitting a diff into ordered review groups.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitPlan {
    /// Ordered list of review groups.
    pub groups: Vec<SplitGroup>,
    /// Files that were intentionally skipped (lockfiles, generated, etc.).
    pub skipped_files: Vec<String>,
    /// Any warnings produced during splitting.
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Review / session types
// ---------------------------------------------------------------------------

/// A single round of review for a branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRound {
    /// Monotonically increasing version number (1, 2, ...).
    pub version: u32,
    /// Git SHA reviewed in this round.
    pub sha: String,
    /// When this round was started.
    pub timestamp: DateTime<Utc>,
    /// Paths of files the reviewer marked as viewed.
    pub files_viewed: Vec<String>,
    /// Number of notes left in this round.
    pub notes_count: usize,
}

/// Persistent review session state for a branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewSession {
    /// Branch name.
    pub branch: String,
    /// History of review rounds.
    pub reviews: Vec<ReviewRound>,
}

/// A single review note attached to a file / line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewNote {
    /// Unique identifier for the note.
    pub id: String,
    /// File path the note is attached to.
    pub file: String,
    /// Optional line number within the file.
    pub line: Option<u32>,
    /// Free-form content of the note.
    pub content: String,
    /// When the note was created.
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    /// Helper: serialise to JSON and deserialise back, asserting no data loss.
    fn round_trip<T>(value: &T)
    where
        T: Serialize + for<'de> Deserialize<'de> + std::fmt::Debug,
    {
        let json = serde_json::to_string_pretty(value).expect("serialize");
        let _back: T = serde_json::from_str(&json).expect("deserialize");
    }

    #[test]
    fn round_trip_diff_file() {
        let file = DiffFile {
            path: "src/main.rs".into(),
            old_path: None,
            status: FileStatus::Modified,
            category: FileCategory::Source,
            hunks: vec![Hunk {
                old_start: 1,
                old_count: 5,
                new_start: 1,
                new_count: 7,
                lines: vec![
                    DiffLine {
                        kind: LineKind::Context,
                        content: "fn main() {".into(),
                        old_line_no: Some(1),
                        new_line_no: Some(1),
                    },
                    DiffLine {
                        kind: LineKind::Added,
                        content: "    println!(\"hello\");".into(),
                        old_line_no: None,
                        new_line_no: Some(2),
                    },
                ],
                classification: HunkClassification::ModifiedLogic,
            }],
            stats: FileStats {
                additions: 2,
                deletions: 0,
                logic_lines: 1,
            },
            priority: ReviewPriority::Deep,
            priority_score: 85.0,
        };
        round_trip(&file);
    }

    #[test]
    fn round_trip_diff_result() {
        let mut files_by_priority = HashMap::new();
        files_by_priority.insert(ReviewPriority::Deep, 1);
        files_by_priority.insert(ReviewPriority::Skip, 2);

        let result = DiffResult {
            base_ref: "abc123".into(),
            head_ref: "def456".into(),
            files: vec![],
            summary: DiffSummary {
                total_files: 3,
                files_by_priority,
                total_additions: 100,
                total_deletions: 20,
                logic_lines_added: 60,
                moved_lines: 10,
                estimated_review_minutes: 15,
            },
        };
        round_trip(&result);
    }

    #[test]
    fn round_trip_review_session() {
        let session = ReviewSession {
            branch: "feature/new-thing".into(),
            reviews: vec![ReviewRound {
                version: 1,
                sha: "abcdef1234567890".into(),
                timestamp: Utc::now(),
                files_viewed: vec!["src/lib.rs".into()],
                notes_count: 3,
            }],
        };
        round_trip(&session);
    }

    #[test]
    fn round_trip_split_plan() {
        let plan = SplitPlan {
            groups: vec![SplitGroup {
                index: 0,
                name: "Core types".into(),
                files: vec!["src/types.rs".into(), "src/errors.rs".into()],
                depends_on: vec![],
                stats: GroupStats {
                    total_files: 2,
                    total_additions: 150,
                    total_deletions: 0,
                    logic_lines: 120,
                },
            }],
            skipped_files: vec!["Cargo.lock".into()],
            warnings: vec![],
        };
        round_trip(&plan);
    }

    #[test]
    fn default_diff_summary_is_zeroed() {
        let s = DiffSummary::default();
        assert_eq!(s.total_files, 0);
        assert_eq!(s.total_additions, 0);
        assert!(s.files_by_priority.is_empty());
    }

    #[test]
    fn default_diff_result_is_empty() {
        let r = DiffResult::default();
        assert!(r.base_ref.is_empty());
        assert!(r.files.is_empty());
    }

    #[test]
    fn hunk_classification_moved_round_trip() {
        let hc = HunkClassification::Moved {
            from: "src/old.rs".into(),
            similarity: 0.95,
        };
        let json = serde_json::to_string(&hc).unwrap();
        let back: HunkClassification = serde_json::from_str(&json).unwrap();
        assert_eq!(hc, back);
    }
}
