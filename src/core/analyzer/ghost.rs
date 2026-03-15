#![allow(dead_code)]

use std::path::Path;

use crate::core::graph::dependency::DependencyGraph;
use crate::core::types::*;

/// Run ghost analysis on a set of changed files against the dependency graph.
///
/// Checks performed:
/// 1. BrokenImport (Error): For each deleted file, find all files that import it
/// 2. MissingTest (Warning): For each changed source file, check if a test file exists
/// 3. HighImpact (Info): For each changed file with >5 dependents
pub fn analyze_ghost(
    changed_files: &[DiffFile],
    all_parsed: &[ParsedFile],
    graph: &DependencyGraph,
) -> GhostResult {
    let mut findings = Vec::new();

    for diff_file in changed_files {
        // 1. BrokenImport: deleted files that are still imported
        if diff_file.status == FileStatus::Deleted {
            let dependents = graph.dependents(&diff_file.path);
            for dep in dependents {
                findings.push(GhostFinding {
                    file: diff_file.path.clone(),
                    severity: GhostSeverity::Error,
                    category: GhostCategory::BrokenImport,
                    message: format!(
                        "Deleted file `{}` is still imported by `{}`",
                        diff_file.path, dep
                    ),
                    related_file: Some(dep.to_string()),
                });
            }
        }

        // 2. MissingTest: source files without a corresponding test
        if diff_file.category == FileCategory::Source {
            let has_test = has_corresponding_test(&diff_file.path, all_parsed);
            if !has_test {
                findings.push(GhostFinding {
                    file: diff_file.path.clone(),
                    severity: GhostSeverity::Warning,
                    category: GhostCategory::MissingTest,
                    message: format!(
                        "Changed source file `{}` has no corresponding test file",
                        diff_file.path
                    ),
                    related_file: None,
                });
            }
        }

        // 3. HighImpact: files with many dependents
        let dependents = graph.dependents(&diff_file.path);
        let dep_count = dependents.len();
        if dep_count > 5 {
            findings.push(GhostFinding {
                file: diff_file.path.clone(),
                severity: GhostSeverity::Info,
                category: GhostCategory::HighImpact {
                    dependent_count: dep_count,
                },
                message: format!(
                    "High-impact change: `{}` is imported by {} other files",
                    diff_file.path, dep_count
                ),
                related_file: None,
            });
        }
    }

    let error_count = findings
        .iter()
        .filter(|f| f.severity == GhostSeverity::Error)
        .count();
    let warning_count = findings
        .iter()
        .filter(|f| f.severity == GhostSeverity::Warning)
        .count();
    let info_count = findings
        .iter()
        .filter(|f| f.severity == GhostSeverity::Info)
        .count();

    GhostResult {
        findings,
        error_count,
        warning_count,
        info_count,
    }
}

/// Check if any file in `all_parsed` looks like a test for the given source path.
///
/// Heuristic: extract the file stem (e.g. `helpers` from `src/utils/helpers.ts`)
/// and check if any parsed file's path contains `{stem}.test.`, `{stem}.spec.`,
/// or `{stem}_test.`.
fn has_corresponding_test(source_path: &str, all_parsed: &[ParsedFile]) -> bool {
    let path = Path::new(source_path);
    let stem = match path.file_stem().and_then(|s| s.to_str()) {
        Some(s) => s,
        None => return false,
    };

    let test_patterns = [
        format!("{}.test.", stem),
        format!("{}.spec.", stem),
        format!("{}_test.", stem),
        format!("{}_test.rs", stem),
    ];

    all_parsed.iter().any(|parsed| {
        // Don't match the source file itself
        if parsed.path == source_path {
            return false;
        }
        let parsed_filename = Path::new(&parsed.path)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");
        test_patterns
            .iter()
            .any(|pattern| parsed_filename.contains(pattern.as_str()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::graph::dependency::DependencyGraph;

    fn make_diff_file(path: &str, status: FileStatus, category: FileCategory) -> DiffFile {
        DiffFile {
            path: path.to_string(),
            old_path: None,
            status,
            category,
            hunks: vec![],
            stats: FileStats::default(),
            priority: ReviewPriority::Scan,
            priority_score: 10.0,
            semantic_diff: None,
        }
    }

    fn make_parsed(path: &str, imports: Vec<(&str, Option<&str>)>) -> ParsedFile {
        ParsedFile {
            path: path.to_string(),
            language: Language::TypeScript,
            imports: imports
                .into_iter()
                .map(|(source, resolved)| Import {
                    source: source.to_string(),
                    names: vec![],
                    default: None,
                    resolved_path: resolved.map(|r| r.to_string()),
                })
                .collect(),
            exports: vec![],
            functions: vec![],
        }
    }

    #[test]
    fn deleted_file_with_importers_produces_broken_import() {
        let changed = vec![make_diff_file(
            "src/utils.ts",
            FileStatus::Deleted,
            FileCategory::Source,
        )];
        let parsed = vec![
            make_parsed("src/app.ts", vec![("./utils", Some("src/utils.ts"))]),
            make_parsed("src/utils.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&parsed);
        let result = analyze_ghost(&changed, &parsed, &graph);
        assert_eq!(result.error_count, 1);
        assert!(result.findings.iter().any(
            |f| f.severity == GhostSeverity::Error && f.category == GhostCategory::BrokenImport
        ));
    }

    #[test]
    fn changed_file_without_test_produces_missing_test() {
        let changed = vec![make_diff_file(
            "src/utils.ts",
            FileStatus::Modified,
            FileCategory::Source,
        )];
        let parsed = vec![make_parsed("src/utils.ts", vec![])];
        let graph = DependencyGraph::build(&parsed);
        let result = analyze_ghost(&changed, &parsed, &graph);
        assert!(result
            .findings
            .iter()
            .any(|f| f.severity == GhostSeverity::Warning
                && f.category == GhostCategory::MissingTest));
    }

    #[test]
    fn changed_file_with_test_no_missing_test_warning() {
        let changed = vec![make_diff_file(
            "src/utils.ts",
            FileStatus::Modified,
            FileCategory::Source,
        )];
        let parsed = vec![
            make_parsed("src/utils.ts", vec![]),
            make_parsed("src/utils.test.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&parsed);
        let result = analyze_ghost(&changed, &parsed, &graph);
        assert!(!result
            .findings
            .iter()
            .any(|f| f.category == GhostCategory::MissingTest));
    }

    #[test]
    fn high_impact_file_produces_info() {
        let changed = vec![make_diff_file(
            "src/core.ts",
            FileStatus::Modified,
            FileCategory::Source,
        )];
        // Create 6+ files that import core.ts
        let mut parsed = vec![make_parsed("src/core.ts", vec![])];
        for i in 0..6 {
            parsed.push(make_parsed(
                &format!("src/consumer{}.ts", i),
                vec![("./core", Some("src/core.ts"))],
            ));
        }
        let graph = DependencyGraph::build(&parsed);
        let result = analyze_ghost(&changed, &parsed, &graph);
        assert!(result
            .findings
            .iter()
            .any(|f| f.severity == GhostSeverity::Info
                && matches!(f.category, GhostCategory::HighImpact { .. })));
    }

    #[test]
    fn no_issues_empty_findings() {
        let changed = vec![make_diff_file(
            "src/utils.ts",
            FileStatus::Modified,
            FileCategory::Source,
        )];
        let parsed = vec![
            make_parsed("src/utils.ts", vec![]),
            make_parsed("src/utils.test.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&parsed);
        let result = analyze_ghost(&changed, &parsed, &graph);
        assert_eq!(result.error_count, 0);
    }
}
