use std::collections::{HashMap, HashSet};

use crate::core::graph::dependency::DependencyGraph;
use crate::core::graph::scc::find_cycles;
use crate::core::graph::topological::topological_sort;
use crate::core::types::{DiffFile, FileCategory, GroupStats, SplitGroup, SplitPlan};
use crate::state::config::SplitConfig;

/// Generate a split plan from changed files and their dependency graph.
pub fn generate_split_plan(
    files: &[DiffFile],
    graph: &DependencyGraph,
    config: &SplitConfig,
) -> SplitPlan {
    let (splittable, skipped_files) = partition_files(files);

    if splittable.is_empty() {
        return SplitPlan {
            groups: vec![],
            skipped_files,
            warnings: vec![],
        };
    }

    let file_paths: HashSet<String> = splittable.iter().map(|f| f.path.clone()).collect();
    let ordered = build_file_order(graph, &file_paths);

    let file_map: HashMap<String, &DiffFile> =
        splittable.iter().map(|f| (f.path.clone(), *f)).collect();

    let mut groups = assign_groups(&ordered, &file_map, config);
    compute_group_deps(&mut groups, graph);

    let warnings = vec![];
    SplitPlan {
        groups,
        skipped_files,
        warnings,
    }
}

/// Separate files into "to split" and "skipped".
/// Skips Lock, Generated, and Snapshot categories.
fn partition_files(files: &[DiffFile]) -> (Vec<&DiffFile>, Vec<String>) {
    let mut remaining = Vec::new();
    let mut skipped = Vec::new();

    for file in files {
        match file.category {
            FileCategory::Lock | FileCategory::Generated | FileCategory::Snapshot => {
                skipped.push(file.path.clone());
            }
            _ => remaining.push(file),
        }
    }

    (remaining, skipped)
}

/// Build a dependency-respecting ordering of files.
/// Uses topological sort when possible, falls back to SCC-aware ordering.
fn build_file_order(graph: &DependencyGraph, file_paths: &HashSet<String>) -> Vec<String> {
    let mut ordered = match topological_sort(graph) {
        Ok(sorted) => sorted
            .into_iter()
            .filter(|p| file_paths.contains(p))
            .collect::<Vec<_>>(),
        Err(_) => build_cycle_aware_order(graph, file_paths),
    };

    // Append any files not present in the graph, sorted alphabetically
    let placed: HashSet<&str> = ordered.iter().map(|s| s.as_str()).collect();
    let mut extra: Vec<String> = file_paths
        .iter()
        .filter(|f| !placed.contains(f.as_str()))
        .cloned()
        .collect();
    extra.sort();
    ordered.extend(extra);

    ordered
}

/// When cycles exist, group SCC members together then sort remaining alphabetically.
fn build_cycle_aware_order(graph: &DependencyGraph, file_paths: &HashSet<String>) -> Vec<String> {
    let sccs = find_cycles(graph);
    let mut placed: HashSet<String> = HashSet::new();
    let mut result = Vec::new();

    // Place SCC members as contiguous blocks
    for scc in &sccs {
        let relevant: Vec<&String> = scc
            .files
            .iter()
            .filter(|f| file_paths.contains(*f))
            .collect();
        if relevant.is_empty() {
            continue;
        }
        for f in relevant {
            if placed.insert(f.clone()) {
                result.push(f.clone());
            }
        }
    }

    // Remaining files sorted alphabetically
    let mut remaining: Vec<String> = file_paths
        .iter()
        .filter(|f| !placed.contains(*f))
        .cloned()
        .collect();
    remaining.sort();
    result.extend(remaining);

    result
}

/// Walk files in order and accumulate into groups respecting target size.
fn assign_groups(
    ordered_files: &[String],
    file_map: &HashMap<String, &DiffFile>,
    config: &SplitConfig,
) -> Vec<SplitGroup> {
    let mut groups: Vec<SplitGroup> = Vec::new();
    let mut current_files: Vec<String> = Vec::new();
    let mut current_size: usize = 0;

    for path in ordered_files {
        let size = file_size(file_map.get(path));
        let would_exceed = current_size + size > config.target_group_size;

        if would_exceed && !current_files.is_empty() {
            groups.push(build_group(groups.len(), &current_files, file_map));
            current_files.clear();
            current_size = 0;
        }

        current_files.push(path.clone());
        current_size += size;
    }

    if !current_files.is_empty() {
        groups.push(build_group(groups.len(), &current_files, file_map));
    }

    groups
}

/// Get the size metric for a file: logic_lines, falling back to additions.
fn file_size(diff: Option<&&DiffFile>) -> usize {
    match diff {
        Some(f) if f.stats.logic_lines > 0 => f.stats.logic_lines,
        Some(f) => f.stats.additions,
        None => 0,
    }
}

/// Build a SplitGroup from a list of file paths.
fn build_group(
    index: usize,
    files: &[String],
    file_map: &HashMap<String, &DiffFile>,
) -> SplitGroup {
    let name = name_group(files, index);
    let stats = compute_group_stats_from_map(files, file_map);
    SplitGroup {
        index,
        name,
        files: files.to_vec(),
        depends_on: vec![],
        stats,
    }
}

/// For each group, compute which earlier groups it depends on.
pub fn compute_group_deps(groups: &mut [SplitGroup], graph: &DependencyGraph) {
    // Build file -> group index mapping (owned keys to avoid borrow issues)
    let file_to_group: HashMap<String, usize> = groups
        .iter()
        .flat_map(|g| g.files.iter().map(move |f| (f.clone(), g.index)))
        .collect();

    for (i, group) in groups.iter_mut().enumerate() {
        let mut deps: HashSet<usize> = HashSet::new();
        for file in &group.files {
            for dep in graph.dependencies(file) {
                if let Some(&group_idx) = file_to_group.get(dep) {
                    if group_idx < i {
                        deps.insert(group_idx);
                    }
                }
            }
        }
        let mut dep_vec: Vec<usize> = deps.into_iter().collect();
        dep_vec.sort();
        group.depends_on = dep_vec;
    }
}

/// Name a group based on the longest common directory prefix of its files.
fn name_group(files: &[String], index: usize) -> String {
    if files.is_empty() {
        return format!("group-{index}");
    }

    let dirs: Vec<&str> = files
        .iter()
        .filter_map(|f| {
            let p = f.rfind('/');
            p.map(|i| &f[..i])
        })
        .collect();

    if dirs.is_empty() || dirs.len() != files.len() {
        return format!("group-{index}");
    }

    let prefix = longest_common_prefix(&dirs);
    if prefix.is_empty() {
        return format!("group-{index}");
    }

    // Strip leading "src/" for a cleaner name
    let name = prefix.strip_prefix("src/").unwrap_or(prefix);
    name.to_string()
}

/// Find the longest common directory prefix among a list of directory paths.
fn longest_common_prefix<'a>(paths: &[&'a str]) -> &'a str {
    if paths.is_empty() {
        return "";
    }

    let first = paths[0];
    let mut end = first.len();

    for path in &paths[1..] {
        end = end.min(path.len());
        for (i, (a, b)) in first.bytes().zip(path.bytes()).enumerate() {
            if i >= end || a != b {
                end = i;
                break;
            }
        }
    }

    let matched = &first[..end];

    // If the match covers a full path segment (ends at string end or at '/'), use it
    if end == first.len() || first.as_bytes().get(end) == Some(&b'/') {
        return matched;
    }

    // Otherwise trim to last '/' boundary for a clean directory prefix
    match matched.rfind('/') {
        Some(i) => &first[..i],
        None => "",
    }
}

/// Compute aggregate stats for a group from a file map.
fn compute_group_stats_from_map(
    files: &[String],
    file_map: &HashMap<String, &DiffFile>,
) -> GroupStats {
    let mut stats = GroupStats::default();
    for path in files {
        if let Some(f) = file_map.get(path) {
            stats.total_files += 1;
            stats.total_additions += f.stats.additions;
            stats.total_deletions += f.stats.deletions;
            stats.logic_lines += f.stats.logic_lines;
        }
    }
    stats
}

/// Compute aggregate stats for a group from the full list of DiffFiles.
pub fn compute_group_stats(group_files: &[String], all_files: &[DiffFile]) -> GroupStats {
    let file_map: HashMap<&str, &DiffFile> =
        all_files.iter().map(|f| (f.path.as_str(), f)).collect();
    let mut stats = GroupStats::default();
    for path in group_files {
        if let Some(f) = file_map.get(path.as_str()) {
            stats.total_files += 1;
            stats.total_additions += f.stats.additions;
            stats.total_deletions += f.stats.deletions;
            stats.logic_lines += f.stats.logic_lines;
        }
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::graph::dependency::DependencyGraph;
    use crate::core::types::*;

    fn make_diff_file(path: &str, category: FileCategory, logic_lines: usize) -> DiffFile {
        DiffFile {
            path: path.to_string(),
            old_path: None,
            status: FileStatus::Modified,
            category,
            hunks: vec![],
            stats: FileStats {
                additions: logic_lines,
                deletions: 0,
                logic_lines,
            },
            priority: ReviewPriority::Scan,
            priority_score: 10.0,
        }
    }

    fn make_parsed(path: &str, imports: Vec<(&str, Option<&str>)>) -> ParsedFile {
        ParsedFile {
            path: path.to_string(),
            language: Language::TypeScript,
            imports: imports
                .into_iter()
                .map(|(s, r)| Import {
                    source: s.to_string(),
                    names: vec![],
                    default: None,
                    resolved_path: r.map(|x| x.to_string()),
                })
                .collect(),
            exports: vec![],
            functions: vec![],
        }
    }

    #[test]
    fn skip_lockfiles() {
        let files = vec![
            make_diff_file("Cargo.lock", FileCategory::Lock, 100),
            make_diff_file("src/main.rs", FileCategory::Source, 50),
        ];
        let graph = DependencyGraph::build(&[]);
        let config = SplitConfig {
            target_group_size: 150,
            max_group_size: 400,
        };
        let plan = generate_split_plan(&files, &graph, &config);
        assert_eq!(plan.skipped_files, vec!["Cargo.lock"]);
        assert_eq!(plan.groups.len(), 1);
        assert_eq!(plan.groups[0].files, vec!["src/main.rs"]);
    }

    #[test]
    fn single_file_one_group() {
        let files = vec![make_diff_file("src/main.rs", FileCategory::Source, 50)];
        let graph = DependencyGraph::build(&[]);
        let config = SplitConfig {
            target_group_size: 150,
            max_group_size: 400,
        };
        let plan = generate_split_plan(&files, &graph, &config);
        assert_eq!(plan.groups.len(), 1);
    }

    #[test]
    fn respects_target_size() {
        let files = vec![
            make_diff_file("a.rs", FileCategory::Source, 100),
            make_diff_file("b.rs", FileCategory::Source, 100),
            make_diff_file("c.rs", FileCategory::Source, 100),
        ];
        let graph = DependencyGraph::build(&[]);
        let config = SplitConfig {
            target_group_size: 150,
            max_group_size: 400,
        };
        let plan = generate_split_plan(&files, &graph, &config);
        // 3 files at 100 lines each, target 150: should get 2 groups
        assert!(plan.groups.len() >= 2);
    }

    #[test]
    fn dependency_order_preserved() {
        // a.ts depends on b.ts: b.ts should be in an earlier group
        let diff_files = vec![
            make_diff_file("a.ts", FileCategory::Source, 100),
            make_diff_file("b.ts", FileCategory::Source, 100),
        ];
        let parsed = vec![
            make_parsed("a.ts", vec![("./b", Some("b.ts"))]),
            make_parsed("b.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&parsed);
        let config = SplitConfig {
            target_group_size: 80,
            max_group_size: 400,
        };
        let plan = generate_split_plan(&diff_files, &graph, &config);
        // b.ts should be in group 0, a.ts in group 1
        if plan.groups.len() >= 2 {
            assert!(plan.groups[0].files.contains(&"b.ts".to_string()));
            assert!(plan.groups[1].files.contains(&"a.ts".to_string()));
        }
    }

    #[test]
    fn group_deps_computed() {
        let diff_files = vec![
            make_diff_file("a.ts", FileCategory::Source, 100),
            make_diff_file("b.ts", FileCategory::Source, 100),
        ];
        let parsed = vec![
            make_parsed("a.ts", vec![("./b", Some("b.ts"))]),
            make_parsed("b.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&parsed);
        let config = SplitConfig {
            target_group_size: 80,
            max_group_size: 400,
        };
        let plan = generate_split_plan(&diff_files, &graph, &config);
        if plan.groups.len() >= 2 {
            // Group containing a.ts should depend on group containing b.ts
            let a_group = plan
                .groups
                .iter()
                .find(|g| g.files.contains(&"a.ts".to_string()))
                .unwrap();
            assert!(!a_group.depends_on.is_empty());
        }
    }

    #[test]
    fn empty_input_empty_plan() {
        let graph = DependencyGraph::build(&[]);
        let config = SplitConfig {
            target_group_size: 150,
            max_group_size: 400,
        };
        let plan = generate_split_plan(&[], &graph, &config);
        assert!(plan.groups.is_empty());
        assert!(plan.skipped_files.is_empty());
    }

    #[test]
    fn skip_generated_and_snapshot() {
        let files = vec![
            make_diff_file("gen.rs", FileCategory::Generated, 50),
            make_diff_file("snap.rs", FileCategory::Snapshot, 30),
            make_diff_file("src/lib.rs", FileCategory::Source, 60),
        ];
        let graph = DependencyGraph::build(&[]);
        let config = SplitConfig {
            target_group_size: 150,
            max_group_size: 400,
        };
        let plan = generate_split_plan(&files, &graph, &config);
        assert_eq!(plan.skipped_files.len(), 2);
        assert!(plan.skipped_files.contains(&"gen.rs".to_string()));
        assert!(plan.skipped_files.contains(&"snap.rs".to_string()));
        assert_eq!(plan.groups.len(), 1);
    }

    #[test]
    fn group_stats_computed_correctly() {
        let files = vec![
            make_diff_file("a.rs", FileCategory::Source, 50),
            make_diff_file("b.rs", FileCategory::Source, 70),
        ];
        let graph = DependencyGraph::build(&[]);
        let config = SplitConfig {
            target_group_size: 200,
            max_group_size: 400,
        };
        let plan = generate_split_plan(&files, &graph, &config);
        assert_eq!(plan.groups.len(), 1);
        assert_eq!(plan.groups[0].stats.total_files, 2);
        assert_eq!(plan.groups[0].stats.total_additions, 120);
        assert_eq!(plan.groups[0].stats.logic_lines, 120);
    }

    #[test]
    fn name_group_common_prefix() {
        let files = vec![
            "src/core/parser/lexer.rs".to_string(),
            "src/core/parser/ast.rs".to_string(),
        ];
        let name = name_group(&files, 0);
        assert_eq!(name, "core/parser");
    }

    #[test]
    fn name_group_no_common_prefix() {
        let files = vec!["a.rs".to_string(), "b.rs".to_string()];
        let name = name_group(&files, 3);
        assert_eq!(name, "group-3");
    }

    #[test]
    fn compute_group_stats_public_api() {
        let all_files = vec![
            make_diff_file("a.rs", FileCategory::Source, 30),
            make_diff_file("b.rs", FileCategory::Source, 50),
            make_diff_file("c.rs", FileCategory::Source, 20),
        ];
        let group_files = vec!["a.rs".to_string(), "c.rs".to_string()];
        let stats = compute_group_stats(&group_files, &all_files);
        assert_eq!(stats.total_files, 2);
        assert_eq!(stats.total_additions, 50);
        assert_eq!(stats.logic_lines, 50);
    }

    #[test]
    fn all_files_skipped_yields_empty_plan() {
        let files = vec![
            make_diff_file("Cargo.lock", FileCategory::Lock, 100),
            make_diff_file("gen.rs", FileCategory::Generated, 50),
        ];
        let graph = DependencyGraph::build(&[]);
        let config = SplitConfig {
            target_group_size: 150,
            max_group_size: 400,
        };
        let plan = generate_split_plan(&files, &graph, &config);
        assert!(plan.groups.is_empty());
        assert_eq!(plan.skipped_files.len(), 2);
    }
}
