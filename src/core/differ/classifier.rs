use crate::core::{Hunk, HunkClassification, LineKind};

pub fn classify_hunk(hunk: &Hunk) -> HunkClassification {
    let changed: Vec<_> = hunk
        .lines
        .iter()
        .filter(|l| matches!(l.kind, LineKind::Added | LineKind::Removed))
        .collect();

    if changed.is_empty() {
        return HunkClassification::WhitespaceOnly;
    }

    if changed.iter().all(|l| l.content.trim().is_empty()) {
        return HunkClassification::WhitespaceOnly;
    }

    let non_blank_changed: Vec<_> = changed
        .iter()
        .filter(|l| !l.content.trim().is_empty())
        .collect();

    if non_blank_changed.iter().all(|l| is_import_line(&l.content)) {
        return HunkClassification::ImportOnly;
    }

    let has_removed = changed.iter().any(|l| matches!(l.kind, LineKind::Removed));

    let has_import = non_blank_changed.iter().any(|l| is_import_line(&l.content));

    if !has_removed && !has_import {
        return HunkClassification::NewLogic;
    }

    HunkClassification::ModifiedLogic
}

fn is_import_line(content: &str) -> bool {
    let trimmed = content.trim();

    trimmed.starts_with("import ")
        || (trimmed.starts_with("from ") && trimmed.contains(" import "))
        || trimmed.contains("require(")
        || trimmed.starts_with("use ")
        || trimmed.starts_with("#include")
        || (trimmed.starts_with("export ") && trimmed.contains(" from "))
}

#[cfg(test)]
mod tests {
    use crate::core::DiffLine;

    use super::*;

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
    fn all_additions_is_new_logic() {
        let hunk = make_hunk(vec![
            (LineKind::Added, "fn new_function() {"),
            (LineKind::Added, "    do_stuff();"),
            (LineKind::Added, "}"),
        ]);
        assert_eq!(classify_hunk(&hunk), HunkClassification::NewLogic);
    }

    #[test]
    fn mixed_add_remove_is_modified_logic() {
        let hunk = make_hunk(vec![
            (LineKind::Context, "fn existing() {"),
            (LineKind::Removed, "    old_call();"),
            (LineKind::Added, "    new_call();"),
            (LineKind::Context, "}"),
        ]);
        assert_eq!(classify_hunk(&hunk), HunkClassification::ModifiedLogic);
    }

    #[test]
    fn only_blank_changes_is_whitespace_only() {
        let hunk = make_hunk(vec![
            (LineKind::Removed, ""),
            (LineKind::Added, "  "),
            (LineKind::Removed, "   "),
            (LineKind::Added, ""),
        ]);
        assert_eq!(classify_hunk(&hunk), HunkClassification::WhitespaceOnly);
    }

    #[test]
    fn js_imports_only_is_import_only() {
        let hunk = make_hunk(vec![
            (LineKind::Removed, "import { foo } from './old';"),
            (LineKind::Added, "import { foo } from './new';"),
            (LineKind::Added, "import { bar } from './bar';"),
        ]);
        assert_eq!(classify_hunk(&hunk), HunkClassification::ImportOnly);
    }

    #[test]
    fn rust_use_only_is_import_only() {
        let hunk = make_hunk(vec![
            (LineKind::Added, "use std::collections::HashMap;"),
            (LineKind::Added, "use crate::core::types::DiffFile;"),
        ]);
        assert_eq!(classify_hunk(&hunk), HunkClassification::ImportOnly);
    }

    #[test]
    fn python_import_only_is_import_only() {
        let hunk = make_hunk(vec![
            (LineKind::Added, "from os.path import join"),
            (LineKind::Removed, "import sys"),
            (LineKind::Added, "import os"),
        ]);
        assert_eq!(classify_hunk(&hunk), HunkClassification::ImportOnly);
    }

    #[test]
    fn mixed_import_and_logic_is_modified_logic() {
        let hunk = make_hunk(vec![
            (LineKind::Added, "import { bar } from './bar';"),
            (LineKind::Added, "const x = bar();"),
        ]);
        assert_eq!(classify_hunk(&hunk), HunkClassification::ModifiedLogic);
    }

    #[test]
    fn context_only_hunk_has_no_changes() {
        let hunk = make_hunk(vec![
            (LineKind::Context, "fn existing() {"),
            (LineKind::Context, "    unchanged();"),
            (LineKind::Context, "}"),
        ]);
        assert_eq!(classify_hunk(&hunk), HunkClassification::WhitespaceOnly);
    }

    #[test]
    fn require_calls_detected_as_imports() {
        let hunk = make_hunk(vec![
            (LineKind::Added, "const fs = require('fs');"),
            (LineKind::Added, "const path = require('path');"),
        ]);
        assert_eq!(classify_hunk(&hunk), HunkClassification::ImportOnly);
    }

    #[test]
    fn c_includes_detected_as_imports() {
        let hunk = make_hunk(vec![
            (LineKind::Added, "#include <stdio.h>"),
            (LineKind::Removed, "#include <stdlib.h>"),
        ]);
        assert_eq!(classify_hunk(&hunk), HunkClassification::ImportOnly);
    }
}
