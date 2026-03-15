use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction;
use std::collections::HashMap;

use crate::core::types::ParsedFile;

/// A directed dependency graph where nodes are file paths
/// and edges represent "A imports B" relationships.
pub struct DependencyGraph {
    graph: DiGraph<String, ()>,
    node_map: HashMap<String, NodeIndex>,
}

#[allow(dead_code)]
impl DependencyGraph {
    /// Build a dependency graph from a set of parsed files.
    /// Only imports with `resolved_path` set create edges.
    pub fn build(files: &[ParsedFile]) -> Self {
        let mut graph = DiGraph::new();
        let mut node_map = HashMap::new();

        // Step 1: Add a node for each parsed file's path.
        for file in files {
            let idx = graph.add_node(file.path.clone());
            node_map.insert(file.path.clone(), idx);
        }

        // Step 2: For each import with a resolved_path, add an edge from file → target.
        for file in files {
            let from_idx = node_map[&file.path];
            for import in &file.imports {
                if let Some(ref resolved) = import.resolved_path {
                    // Ensure the target node exists (it may not be in the parsed files list).
                    let to_idx = *node_map
                        .entry(resolved.clone())
                        .or_insert_with(|| graph.add_node(resolved.clone()));
                    graph.add_edge(from_idx, to_idx, ());
                }
            }
        }

        Self { graph, node_map }
    }

    /// Get all direct dependencies of a file (files it imports).
    pub fn dependencies(&self, path: &str) -> Vec<&str> {
        let Some(&idx) = self.node_map.get(path) else {
            return vec![];
        };
        self.graph
            .neighbors_directed(idx, Direction::Outgoing)
            .map(|n| self.graph[n].as_str())
            .collect()
    }

    /// Get all direct dependents of a file (files that import it).
    pub fn dependents(&self, path: &str) -> Vec<&str> {
        let Some(&idx) = self.node_map.get(path) else {
            return vec![];
        };
        self.graph
            .neighbors_directed(idx, Direction::Incoming)
            .map(|n| self.graph[n].as_str())
            .collect()
    }

    /// Get the total number of files in the graph.
    pub fn file_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Get the total number of edges (import relationships).
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Check if file A depends on file B (directly).
    pub fn has_dependency(&self, from: &str, to: &str) -> bool {
        let (Some(&from_idx), Some(&to_idx)) = (self.node_map.get(from), self.node_map.get(to))
        else {
            return false;
        };
        self.graph.contains_edge(from_idx, to_idx)
    }

    /// Get all files in the graph.
    pub fn files(&self) -> Vec<&str> {
        self.graph
            .node_indices()
            .map(|n| self.graph[n].as_str())
            .collect()
    }

    /// Get a reference to the internal petgraph DiGraph.
    pub fn inner_graph(&self) -> &DiGraph<String, ()> {
        &self.graph
    }

    /// Look up the NodeIndex for a file path.
    pub fn node_index(&self, path: &str) -> Option<NodeIndex> {
        self.node_map.get(path).copied()
    }

    /// Get the file path label for a NodeIndex.
    pub fn node_label(&self, idx: NodeIndex) -> &str {
        &self.graph[idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn build_from_three_files() {
        let files = vec![
            make_file("a.ts", vec![("./b", Some("b.ts"))]),
            make_file("b.ts", vec![("./c", Some("c.ts"))]),
            make_file("c.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&files);
        assert_eq!(graph.file_count(), 3);
        assert_eq!(graph.edge_count(), 2);
    }

    #[test]
    fn dependencies_returns_imports() {
        let files = vec![
            make_file("a.ts", vec![("./b", Some("b.ts")), ("./c", Some("c.ts"))]),
            make_file("b.ts", vec![]),
            make_file("c.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&files);
        let mut deps = graph.dependencies("a.ts");
        deps.sort();
        assert_eq!(deps, vec!["b.ts", "c.ts"]);
    }

    #[test]
    fn dependents_returns_reverse() {
        let files = vec![
            make_file("a.ts", vec![("./c", Some("c.ts"))]),
            make_file("b.ts", vec![("./c", Some("c.ts"))]),
            make_file("c.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&files);
        let mut deps = graph.dependents("c.ts");
        deps.sort();
        assert_eq!(deps, vec!["a.ts", "b.ts"]);
    }

    #[test]
    fn missing_file_returns_empty() {
        let graph = DependencyGraph::build(&[]);
        assert!(graph.dependencies("nonexistent.ts").is_empty());
        assert!(graph.dependents("nonexistent.ts").is_empty());
    }

    #[test]
    fn circular_dependency() {
        let files = vec![
            make_file("a.ts", vec![("./b", Some("b.ts"))]),
            make_file("b.ts", vec![("./a", Some("a.ts"))]),
        ];
        let graph = DependencyGraph::build(&files);
        assert_eq!(graph.edge_count(), 2);
        assert!(graph.has_dependency("a.ts", "b.ts"));
        assert!(graph.has_dependency("b.ts", "a.ts"));
    }

    #[test]
    fn unresolved_imports_ignored() {
        let files = vec![
            make_file("a.ts", vec![("react", None), ("./b", Some("b.ts"))]),
            make_file("b.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&files);
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn file_with_no_imports() {
        let files = vec![make_file("a.ts", vec![])];
        let graph = DependencyGraph::build(&files);
        assert_eq!(graph.file_count(), 1);
        assert!(graph.dependencies("a.ts").is_empty());
    }
}
