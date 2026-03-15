use petgraph::algo::toposort;

use crate::core::graph::dependency::DependencyGraph;

/// Topologically sort files so depended-on files come first.
/// Returns Err with a cycle description if the graph has cycles.
#[allow(dead_code)]
pub fn topological_sort(graph: &DependencyGraph) -> Result<Vec<String>, Vec<String>> {
    match toposort(graph.inner_graph(), None) {
        Ok(indices) => {
            // petgraph returns nodes where each node comes before its outgoing edges.
            // Since edges mean "A imports B" (A->B), this puts importers before their
            // dependencies. We want dependencies first, so reverse.
            let mut result: Vec<String> = indices
                .iter()
                .map(|&idx| graph.node_label(idx).to_string())
                .collect();
            result.reverse();
            Ok(result)
        }
        Err(cycle) => {
            // Return the cycle node as error info
            Err(vec![graph.node_label(cycle.node_id()).to_string()])
        }
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
    fn linear_chain_topo_order() {
        // A imports B, B imports C: result should be [C, B, A]
        let files = vec![
            make_file("a.ts", vec![("./b", Some("b.ts"))]),
            make_file("b.ts", vec![("./c", Some("c.ts"))]),
            make_file("c.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&files);
        let result = topological_sort(&graph).unwrap();
        // C must come before B, B before A
        let pos_c = result.iter().position(|s| s == "c.ts").unwrap();
        let pos_b = result.iter().position(|s| s == "b.ts").unwrap();
        let pos_a = result.iter().position(|s| s == "a.ts").unwrap();
        assert!(pos_c < pos_b);
        assert!(pos_b < pos_a);
    }

    #[test]
    fn diamond_graph() {
        // A->B, A->C, B->D, C->D
        let files = vec![
            make_file("a.ts", vec![("./b", Some("b.ts")), ("./c", Some("c.ts"))]),
            make_file("b.ts", vec![("./d", Some("d.ts"))]),
            make_file("c.ts", vec![("./d", Some("d.ts"))]),
            make_file("d.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&files);
        let result = topological_sort(&graph).unwrap();
        let pos_d = result.iter().position(|s| s == "d.ts").unwrap();
        let pos_a = result.iter().position(|s| s == "a.ts").unwrap();
        assert!(pos_d < pos_a);
    }

    #[test]
    fn single_node() {
        let files = vec![make_file("a.ts", vec![])];
        let graph = DependencyGraph::build(&files);
        let result = topological_sort(&graph).unwrap();
        assert_eq!(result, vec!["a.ts"]);
    }

    #[test]
    fn empty_graph() {
        let graph = DependencyGraph::build(&[]);
        let result = topological_sort(&graph).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn cycle_returns_err() {
        let files = vec![
            make_file("a.ts", vec![("./b", Some("b.ts"))]),
            make_file("b.ts", vec![("./a", Some("a.ts"))]),
        ];
        let graph = DependencyGraph::build(&files);
        assert!(topological_sort(&graph).is_err());
    }
}
