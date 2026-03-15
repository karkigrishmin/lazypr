#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use globset::Glob;

use super::review_session::sanitize_branch_name;
use super::store::store_path;
use crate::core::types::{ChecklistItem, ChecklistRule};

/// Load checklist rules from `.lazypr/checklist.yml`.
/// Returns empty vec if the file does not exist.
pub fn load_checklist_rules(repo_root: &Path) -> Result<Vec<ChecklistRule>> {
    let path = repo_root.join(".lazypr").join("checklist.yml");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let rules: Vec<ChecklistRule> =
        serde_yaml::from_str(&contents).with_context(|| format!("parsing {}", path.display()))?;
    Ok(rules)
}

/// Match changed file paths against checklist rules.
/// Returns a map from file path to applicable checklist items.
pub fn resolve_checklist(
    rules: &[ChecklistRule],
    changed_files: &[String],
) -> HashMap<String, Vec<ChecklistItem>> {
    let mut result: HashMap<String, Vec<ChecklistItem>> = HashMap::new();

    for rule in rules {
        let glob = match Glob::new(&rule.when) {
            Ok(g) => g.compile_matcher(),
            Err(_) => continue, // skip invalid globs
        };

        for file in changed_files {
            if glob.is_match(file) {
                let items = result.entry(file.clone()).or_default();
                for check in &rule.checks {
                    // Deduplicate: don't add the same check text twice
                    if !items.iter().any(|i| i.text == *check) {
                        items.push(ChecklistItem {
                            text: check.clone(),
                            checked: false,
                            source_pattern: rule.when.clone(),
                        });
                    }
                }
            }
        }
    }

    result
}

/// Load persisted checklist state from `.lazypr/reviews/{branch}_checklist.json`.
pub fn load_checklist_state(
    repo_root: &Path,
    branch: &str,
) -> Result<HashMap<String, Vec<ChecklistItem>>> {
    let path = store_path(repo_root)
        .join("reviews")
        .join(format!("{}_checklist.json", sanitize_branch_name(branch)));
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let contents = std::fs::read_to_string(&path)?;
    let state = serde_json::from_str(&contents)?;
    Ok(state)
}

/// Save checklist state.
pub fn save_checklist_state(
    repo_root: &Path,
    branch: &str,
    state: &HashMap<String, Vec<ChecklistItem>>,
) -> Result<()> {
    let path = store_path(repo_root)
        .join("reviews")
        .join(format!("{}_checklist.json", sanitize_branch_name(branch)));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(state)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Merge resolved checklist with saved state (preserving checked status).
pub fn merge_checklist_state(
    resolved: HashMap<String, Vec<ChecklistItem>>,
    saved: &HashMap<String, Vec<ChecklistItem>>,
) -> HashMap<String, Vec<ChecklistItem>> {
    let mut merged = resolved;
    for (file, items) in merged.iter_mut() {
        if let Some(saved_items) = saved.get(file) {
            for item in items.iter_mut() {
                if let Some(saved_item) = saved_items.iter().find(|s| s.text == item.text) {
                    item.checked = saved_item.checked;
                }
            }
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_empty_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let rules = load_checklist_rules(tmp.path()).unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn load_yaml_rules() {
        let tmp = TempDir::new().unwrap();
        let lazypr_dir = tmp.path().join(".lazypr");
        std::fs::create_dir_all(&lazypr_dir).unwrap();
        std::fs::write(
            lazypr_dir.join("checklist.yml"),
            r#"- when: "src/hooks/*"
  checks:
    - "Cleanup in useEffect?"
    - "Tests added?"
"#,
        )
        .unwrap();
        let rules = load_checklist_rules(tmp.path()).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].when, "src/hooks/*");
        assert_eq!(rules[0].checks.len(), 2);
    }

    #[test]
    fn glob_matching_works() {
        let rules = vec![ChecklistRule {
            when: "src/hooks/*".to_string(),
            checks: vec!["Tests added?".to_string()],
        }];
        let files = vec![
            "src/hooks/useAuth.ts".to_string(),
            "src/utils/helpers.ts".to_string(),
        ];
        let result = resolve_checklist(&rules, &files);
        assert!(result.contains_key("src/hooks/useAuth.ts"));
        assert!(!result.contains_key("src/utils/helpers.ts"));
    }

    #[test]
    fn multiple_patterns_match_same_file() {
        let rules = vec![
            ChecklistRule {
                when: "src/**".to_string(),
                checks: vec!["Tests?".to_string()],
            },
            ChecklistRule {
                when: "src/hooks/*".to_string(),
                checks: vec!["Cleanup?".to_string()],
            },
        ];
        let files = vec!["src/hooks/useAuth.ts".to_string()];
        let result = resolve_checklist(&rules, &files);
        let items = result.get("src/hooks/useAuth.ts").unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn merge_preserves_checked() {
        let mut resolved = HashMap::new();
        resolved.insert(
            "a.ts".to_string(),
            vec![ChecklistItem {
                text: "Tests?".to_string(),
                checked: false,
                source_pattern: "**".to_string(),
            }],
        );
        let mut saved = HashMap::new();
        saved.insert(
            "a.ts".to_string(),
            vec![ChecklistItem {
                text: "Tests?".to_string(),
                checked: true,
                source_pattern: "**".to_string(),
            }],
        );
        let merged = merge_checklist_state(resolved, &saved);
        assert!(merged["a.ts"][0].checked);
    }

    #[test]
    fn save_load_round_trip() {
        let tmp = TempDir::new().unwrap();
        crate::state::store::init_store(tmp.path()).unwrap();
        let mut state = HashMap::new();
        state.insert(
            "file.ts".to_string(),
            vec![ChecklistItem {
                text: "Check".to_string(),
                checked: true,
                source_pattern: "**".to_string(),
            }],
        );
        save_checklist_state(tmp.path(), "main", &state).unwrap();
        let loaded = load_checklist_state(tmp.path(), "main").unwrap();
        assert!(loaded["file.ts"][0].checked);
    }
}
