use std::collections::{HashMap, HashSet, VecDeque};

use crate::core::graph::dependency::DependencyGraph;

/// Severity level for an impact finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImpactSeverity {
    /// Direct dependent — imports the changed file.
    Direct,
    /// Transitive dependent — imports something that imports the changed file.
    Transitive { depth: usize },
}

/// A single impact finding.
#[derive(Debug, Clone)]
pub struct ImpactEntry {
    /// Path of the affected file.
    pub file: String,
    /// How this file is affected.
    pub severity: ImpactSeverity,
}

/// Result of an impact analysis for one or more changed files.
#[derive(Debug, Clone)]
pub struct ImpactResult {
    /// The files that were changed (input).
    #[allow(dead_code)]
    pub changed_files: Vec<String>,
    /// All files impacted by the changes.
    pub impacted: Vec<ImpactEntry>,
    /// Total number of directly impacted files.
    pub direct_count: usize,
    /// Total number of transitively impacted files.
    pub transitive_count: usize,
}

/// Compute impact analysis: given a set of changed files,
/// find all files that depend on them (directly or transitively).
pub fn analyze_impact(
    graph: &DependencyGraph,
    changed_files: &[String],
    max_depth: usize,
) -> ImpactResult {
    let changed_set: HashSet<&str> = changed_files.iter().map(|s| s.as_str()).collect();

    // BFS: map from discovered file -> shortest depth
    let mut depths: HashMap<String, usize> = HashMap::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();

    // Seed the queue with all changed files at depth 0.
    for file in changed_files {
        visited.insert(file.clone());
        queue.push_back((file.clone(), 0));
    }

    while let Some((current, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }

        let next_depth = depth + 1;
        for dependent in graph.dependents(&current) {
            let dep_str = dependent.to_string();
            if visited.contains(&dep_str) {
                continue;
            }
            visited.insert(dep_str.clone());
            depths.insert(dep_str.clone(), next_depth);
            queue.push_back((dep_str, next_depth));
        }
    }

    // Remove changed files from the results (they should not appear as impacted).
    for file in &changed_set {
        depths.remove(*file);
    }

    // Build sorted entries: by depth ascending, then alphabetically.
    let mut entries: Vec<(String, usize)> = depths.into_iter().collect();
    entries.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));

    let mut direct_count = 0;
    let mut transitive_count = 0;
    let mut impacted = Vec::with_capacity(entries.len());

    for (file, depth) in entries {
        let severity = if depth == 1 {
            direct_count += 1;
            ImpactSeverity::Direct
        } else {
            transitive_count += 1;
            ImpactSeverity::Transitive { depth }
        };
        impacted.push(ImpactEntry { file, severity });
    }

    ImpactResult {
        changed_files: changed_files.to_vec(),
        impacted,
        direct_count,
        transitive_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::graph::dependency::DependencyGraph;
    use crate::core::types::{Import, Language, ParsedFile};

    fn make_file(path: &str, imports: Vec<(&str, Option<&str>)>) -> ParsedFile {
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
    fn linear_chain_impact() {
        // A -> B -> C: changing C impacts B(direct) and A(transitive depth=2)
        let files = vec![
            make_file("a.ts", vec![("./b", Some("b.ts"))]),
            make_file("b.ts", vec![("./c", Some("c.ts"))]),
            make_file("c.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&files);
        let result = analyze_impact(&graph, &["c.ts".to_string()], 10);
        assert_eq!(result.direct_count, 1);
        assert_eq!(result.transitive_count, 1);
        assert!(result
            .impacted
            .iter()
            .any(|e| e.file == "b.ts" && e.severity == ImpactSeverity::Direct));
        assert!(result
            .impacted
            .iter()
            .any(|e| e.file == "a.ts" && e.severity == ImpactSeverity::Transitive { depth: 2 }));
    }

    #[test]
    fn diamond_impact() {
        // A -> B, A -> C, B -> D, C -> D: changing D impacts B,C(direct), A(transitive)
        let files = vec![
            make_file("a.ts", vec![("./b", Some("b.ts")), ("./c", Some("c.ts"))]),
            make_file("b.ts", vec![("./d", Some("d.ts"))]),
            make_file("c.ts", vec![("./d", Some("d.ts"))]),
            make_file("d.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&files);
        let result = analyze_impact(&graph, &["d.ts".to_string()], 10);
        assert_eq!(result.direct_count, 2); // b.ts, c.ts
        assert_eq!(result.transitive_count, 1); // a.ts
    }

    #[test]
    fn leaf_file_no_impact() {
        let files = vec![
            make_file("a.ts", vec![("./b", Some("b.ts"))]),
            make_file("b.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&files);
        let result = analyze_impact(&graph, &["a.ts".to_string()], 10);
        assert!(result.impacted.is_empty());
        assert_eq!(result.direct_count, 0);
    }

    #[test]
    fn max_depth_limits_traversal() {
        // A -> B -> C -> D: with max_depth=1, changing D only shows C
        let files = vec![
            make_file("a.ts", vec![("./b", Some("b.ts"))]),
            make_file("b.ts", vec![("./c", Some("c.ts"))]),
            make_file("c.ts", vec![("./d", Some("d.ts"))]),
            make_file("d.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&files);
        let result = analyze_impact(&graph, &["d.ts".to_string()], 1);
        assert_eq!(result.impacted.len(), 1);
        assert_eq!(result.impacted[0].file, "c.ts");
    }

    #[test]
    fn circular_dependency_handled() {
        let files = vec![
            make_file("a.ts", vec![("./b", Some("b.ts"))]),
            make_file("b.ts", vec![("./a", Some("a.ts"))]),
        ];
        let graph = DependencyGraph::build(&files);
        let result = analyze_impact(&graph, &["a.ts".to_string()], 10);
        // b.ts imports a.ts, so b.ts is impacted directly
        assert_eq!(result.impacted.len(), 1);
        assert_eq!(result.impacted[0].file, "b.ts");
    }
}
