use std::collections::{HashMap, HashSet};

use crate::core::graph::dependency::DependencyGraph;
use crate::core::types::SplitPlan;

/// A validation issue found in a split plan.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// Index of the group where the issue was found.
    pub group_index: usize,
    /// The file that has the problematic dependency.
    pub file: String,
    /// The dependency that's in the wrong group.
    pub missing_dep: String,
    /// Human-readable description.
    pub message: String,
}

/// Validate that each group's imports resolve within the group
/// or within a group it depends on.
///
/// Returns an empty vec if the plan is valid.
pub fn validate_plan(plan: &SplitPlan, graph: &DependencyGraph) -> Vec<ValidationIssue> {
    // Map from file path -> group index for fast lookup
    let mut file_to_group: HashMap<&str, usize> = HashMap::new();
    for group in &plan.groups {
        for file in &group.files {
            file_to_group.insert(file.as_str(), group.index);
        }
    }

    let mut issues = Vec::new();

    for group in &plan.groups {
        // Build the set of all files "available" to this group:
        // files in the group itself + files in groups it depends on.
        let mut available: HashSet<&str> = HashSet::new();
        for file in &group.files {
            available.insert(file.as_str());
        }
        for &dep_group_idx in &group.depends_on {
            if let Some(dep_group) = plan.groups.iter().find(|g| g.index == dep_group_idx) {
                for file in &dep_group.files {
                    available.insert(file.as_str());
                }
            }
        }

        // Check each file's dependencies
        for file in &group.files {
            for dep in graph.dependencies(file.as_str()) {
                if !available.contains(dep) && file_to_group.contains_key(dep) {
                    // The dependency is in another group that this group doesn't depend on
                    let dep_group_idx = file_to_group[dep];
                    issues.push(ValidationIssue {
                        group_index: group.index,
                        file: file.clone(),
                        missing_dep: dep.to_string(),
                        message: format!(
                            "File '{}' in group {} depends on '{}' which is in group {}, \
                             but group {} does not list group {} in depends_on",
                            file, group.index, dep, dep_group_idx, group.index, dep_group_idx,
                        ),
                    });
                }
            }
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::graph::dependency::DependencyGraph;
    use crate::core::types::*;

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

    fn make_group(index: usize, files: Vec<&str>, depends_on: Vec<usize>) -> SplitGroup {
        SplitGroup {
            index,
            name: format!("group-{}", index),
            files: files.into_iter().map(|s| s.to_string()).collect(),
            depends_on,
            stats: GroupStats::default(),
        }
    }

    #[test]
    fn valid_plan_no_issues() {
        let parsed = vec![
            make_parsed("a.ts", vec![("./b", Some("b.ts"))]),
            make_parsed("b.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&parsed);
        let plan = SplitPlan {
            groups: vec![
                make_group(0, vec!["b.ts"], vec![]),
                make_group(1, vec!["a.ts"], vec![0]),
            ],
            skipped_files: vec![],
            warnings: vec![],
        };
        assert!(validate_plan(&plan, &graph).is_empty());
    }

    #[test]
    fn forward_dep_detected() {
        // a.ts depends on b.ts, but a.ts is in group 0 and b.ts is in group 1
        let parsed = vec![
            make_parsed("a.ts", vec![("./b", Some("b.ts"))]),
            make_parsed("b.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&parsed);
        let plan = SplitPlan {
            groups: vec![
                make_group(0, vec!["a.ts"], vec![]), // a depends on b but b is in later group
                make_group(1, vec!["b.ts"], vec![]),
            ],
            skipped_files: vec![],
            warnings: vec![],
        };
        let issues = validate_plan(&plan, &graph);
        assert!(!issues.is_empty());
        assert_eq!(issues[0].file, "a.ts");
        assert_eq!(issues[0].missing_dep, "b.ts");
    }

    #[test]
    fn cross_group_in_depends_on_ok() {
        // a.ts depends on b.ts, a is in group 1, b in group 0, group 1 depends_on [0]
        let parsed = vec![
            make_parsed("a.ts", vec![("./b", Some("b.ts"))]),
            make_parsed("b.ts", vec![]),
        ];
        let graph = DependencyGraph::build(&parsed);
        let plan = SplitPlan {
            groups: vec![
                make_group(0, vec!["b.ts"], vec![]),
                make_group(1, vec!["a.ts"], vec![0]),
            ],
            skipped_files: vec![],
            warnings: vec![],
        };
        assert!(validate_plan(&plan, &graph).is_empty());
    }

    #[test]
    fn external_deps_ignored() {
        // a.ts depends on "react" (not in any group)
        let parsed = vec![make_parsed("a.ts", vec![("react", None)])];
        let graph = DependencyGraph::build(&parsed);
        let plan = SplitPlan {
            groups: vec![make_group(0, vec!["a.ts"], vec![])],
            skipped_files: vec![],
            warnings: vec![],
        };
        assert!(validate_plan(&plan, &graph).is_empty());
    }

    #[test]
    fn empty_plan_valid() {
        let graph = DependencyGraph::build(&[]);
        let plan = SplitPlan {
            groups: vec![],
            skipped_files: vec![],
            warnings: vec![],
        };
        assert!(validate_plan(&plan, &graph).is_empty());
    }
}
