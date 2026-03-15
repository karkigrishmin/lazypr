#![allow(dead_code)]

use petgraph::algo::tarjan_scc;

use crate::core::graph::dependency::DependencyGraph;

/// A strongly connected component — a group of files with circular dependencies.
#[derive(Debug, Clone)]
pub struct Scc {
    /// Files that form a cycle.
    pub files: Vec<String>,
}

/// Find all strongly connected components with more than one node.
/// Single-node SCCs (no self-loop) are excluded — only actual cycles are returned.
pub fn find_cycles(graph: &DependencyGraph) -> Vec<Scc> {
    let sccs = tarjan_scc(graph.inner_graph());
    sccs.into_iter()
        .filter(|scc| scc.len() > 1)
        .map(|scc| {
            let mut files: Vec<String> = scc
                .iter()
                .map(|&idx| graph.node_label(idx).to_string())
                .collect();
            files.sort(); // deterministic order
            Scc { files }
        })
        .collect()
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
    fn no_cycles_returns_empty() {
        let files = vec![
            make_file("a.ts", vec![("./b", Some("b.ts"))]),
            make_file("b.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&files);
        assert!(find_cycles(&graph).is_empty());
    }

    #[test]
    fn simple_cycle() {
        let files = vec![
            make_file("a.ts", vec![("./b", Some("b.ts"))]),
            make_file("b.ts", vec![("./a", Some("a.ts"))]),
        ];
        let graph = DependencyGraph::build(&files);
        let cycles = find_cycles(&graph);
        assert_eq!(cycles.len(), 1);
        assert!(cycles[0].files.contains(&"a.ts".to_string()));
        assert!(cycles[0].files.contains(&"b.ts".to_string()));
    }

    #[test]
    fn two_independent_cycles() {
        let files = vec![
            make_file("a.ts", vec![("./b", Some("b.ts"))]),
            make_file("b.ts", vec![("./a", Some("a.ts"))]),
            make_file("c.ts", vec![("./d", Some("d.ts"))]),
            make_file("d.ts", vec![("./c", Some("c.ts"))]),
        ];
        let graph = DependencyGraph::build(&files);
        let cycles = find_cycles(&graph);
        assert_eq!(cycles.len(), 2);
    }

    #[test]
    fn three_node_cycle() {
        let files = vec![
            make_file("a.ts", vec![("./b", Some("b.ts"))]),
            make_file("b.ts", vec![("./c", Some("c.ts"))]),
            make_file("c.ts", vec![("./a", Some("a.ts"))]),
        ];
        let graph = DependencyGraph::build(&files);
        let cycles = find_cycles(&graph);
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].files.len(), 3);
    }

    #[test]
    fn cycle_within_larger_graph() {
        let files = vec![
            make_file("a.ts", vec![("./b", Some("b.ts"))]),
            make_file("b.ts", vec![("./a", Some("a.ts"))]),
            make_file("c.ts", vec![("./a", Some("a.ts"))]),
            make_file("d.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&files);
        let cycles = find_cycles(&graph);
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].files.len(), 2);
    }
}
