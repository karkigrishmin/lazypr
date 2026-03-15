use crate::core::types::{Export, ExportKind, FunctionSignature, Import, Language};

use super::LanguageParser;

/// Regex-based fallback parser for languages without a dedicated parser.
/// Uses simple string matching (no regex crate) to catch common patterns
/// across JavaScript, Python, Rust, C/C++, and other languages.
pub struct GenericParser;

impl LanguageParser for GenericParser {
    fn language(&self) -> Language {
        Language::Unknown
    }

    fn can_parse(&self, _extension: &str) -> bool {
        true // fallback parser accepts anything
    }

    fn parse_imports(&self, source: &str) -> Vec<Import> {
        let mut imports = Vec::new();

        for line in source.lines() {
            let trimmed = line.trim();

            // ESM: import ... from "source" or import ... from 'source'
            if trimmed.starts_with("import ") {
                if let Some(imp) = parse_esm_import(trimmed) {
                    imports.push(imp);
                    continue;
                }
            }

            // CommonJS: require("source") or require('source')
            if trimmed.contains("require(") {
                if let Some(imp) = parse_require(trimmed) {
                    imports.push(imp);
                    continue;
                }
            }

            // Python: from module import ...
            if trimmed.starts_with("from ") && trimmed.contains(" import") {
                if let Some(imp) = parse_python_import(trimmed) {
                    imports.push(imp);
                    continue;
                }
            }

            // Rust: use path::to::item;
            if trimmed.starts_with("use ") {
                if let Some(imp) = parse_rust_use(trimmed) {
                    imports.push(imp);
                    continue;
                }
            }

            // C/C++: #include "header" or #include <header>
            if trimmed.starts_with("#include") {
                if let Some(imp) = parse_c_include(trimmed) {
                    imports.push(imp);
                    continue;
                }
            }
        }

        imports
    }

    fn parse_exports(&self, source: &str) -> Vec<Export> {
        let mut exports = Vec::new();

        for line in source.lines() {
            let trimmed = line.trim();

            // export (default)? (function|const|class|let|var) name
            if trimmed.starts_with("export ") {
                if let Some(exp) = parse_js_export(trimmed) {
                    exports.push(exp);
                }
            }
        }

        exports
    }

    fn parse_functions(&self, source: &str) -> Vec<FunctionSignature> {
        let mut functions = Vec::new();

        for (line_idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Rust-like: (pub)? fn name(
            if let Some(func) = parse_rust_fn(trimmed, line_idx as u32 + 1) {
                functions.push(func);
                continue;
            }

            // JS-like: (async)? function name(
            if let Some(func) = parse_js_function(trimmed, line_idx as u32 + 1) {
                functions.push(func);
                continue;
            }

            // Python-like: def name(
            if let Some(func) = parse_python_def(trimmed, line_idx as u32 + 1) {
                functions.push(func);
                continue;
            }
        }

        functions
    }
}

/// Extract the content between matching quote characters starting at `start`.
/// Returns the extracted string if quotes are found.
fn extract_quoted(s: &str, start: usize) -> Option<String> {
    let bytes = s.as_bytes();
    if start >= bytes.len() {
        return None;
    }

    let quote_char = bytes[start];
    if quote_char != b'"' && quote_char != b'\'' {
        return None;
    }

    let rest = &s[start + 1..];
    let end = rest.find(quote_char as char)?;
    Some(rest[..end].to_string())
}

/// Parse ESM import: `import { a, b } from "source"` or `import X from "source"`
fn parse_esm_import(line: &str) -> Option<Import> {
    // Look for `from` followed by a quoted string
    let from_idx = line.rfind(" from ")?;
    let after_from = &line[from_idx + 6..]; // skip " from "
    let after_from = after_from.trim();

    let source = extract_quoted(after_from, 0)?;
    if source.is_empty() {
        return None;
    }

    // Try to extract named imports from `{ name1, name2 }`
    let between = &line["import ".len()..from_idx];
    let between = between.trim();
    let mut names = Vec::new();
    let mut default = None;

    if let Some(brace_start) = between.find('{') {
        if let Some(brace_end) = between.find('}') {
            let inner = &between[brace_start + 1..brace_end];
            for name in inner.split(',') {
                let name = name.trim();
                if !name.is_empty() {
                    // Handle `name as alias` — use the original name
                    let actual = name.split(" as ").next().unwrap_or(name).trim();
                    if !actual.is_empty() {
                        names.push(actual.to_string());
                    }
                }
            }
        }
        // Check for default import before the braces
        let before_brace = between[..between.find('{').unwrap_or(0)].trim();
        let before_brace = before_brace.trim_end_matches(',').trim();
        if !before_brace.is_empty() && before_brace != "*" {
            default = Some(before_brace.to_string());
        }
    } else if between != "*" && !between.starts_with("* as") && !between.is_empty() {
        // Default import only: `import React from "react"`
        let name = between.trim_end_matches(',').trim();
        if !name.is_empty() {
            default = Some(name.to_string());
        }
    }

    Some(Import {
        source,
        names,
        default,
        resolved_path: None,
    })
}

/// Parse CommonJS require: `const x = require("source")`
fn parse_require(line: &str) -> Option<Import> {
    let req_idx = line.find("require(")?;
    let after = &line[req_idx + 8..]; // skip "require("

    let source = extract_quoted(after, 0)?;
    if source.is_empty() {
        return None;
    }

    Some(Import {
        source,
        names: Vec::new(),
        default: None,
        resolved_path: None,
    })
}

/// Parse Python import: `from module import name1, name2`
fn parse_python_import(line: &str) -> Option<Import> {
    // "from module import ..."
    let after_from = line.strip_prefix("from ")?.trim();
    let import_idx = after_from.find(" import")?;
    let source = after_from[..import_idx].trim().to_string();
    if source.is_empty() {
        return None;
    }

    let after_import = &after_from[import_idx + 7..]; // skip " import"
    let after_import = after_import.trim();

    let mut names = Vec::new();
    for name in after_import.split(',') {
        let name = name.trim().trim_end_matches(';');
        if !name.is_empty() {
            // Handle `name as alias`
            let actual = name.split(" as ").next().unwrap_or(name).trim();
            if !actual.is_empty() {
                names.push(actual.to_string());
            }
        }
    }

    Some(Import {
        source,
        names,
        default: None,
        resolved_path: None,
    })
}

/// Parse Rust use: `use std::collections::HashMap;`
fn parse_rust_use(line: &str) -> Option<Import> {
    let after_use = line.strip_prefix("use ")?.trim();
    // Remove trailing semicolons and whitespace
    let source = after_use.trim_end_matches(';').trim();
    if source.is_empty() {
        return None;
    }

    Some(Import {
        source: source.to_string(),
        names: Vec::new(),
        default: None,
        resolved_path: None,
    })
}

/// Parse C/C++ include: `#include "header.h"` or `#include <header.h>`
fn parse_c_include(line: &str) -> Option<Import> {
    let after = line.strip_prefix("#include")?.trim();
    if after.is_empty() {
        return None;
    }

    let source = if after.starts_with('"') {
        extract_quoted(after, 0)?
    } else if after.starts_with('<') {
        let end = after.find('>')?;
        after[1..end].to_string()
    } else {
        return None;
    };

    if source.is_empty() {
        return None;
    }

    Some(Import {
        source,
        names: Vec::new(),
        default: None,
        resolved_path: None,
    })
}

/// Parse JS/TS export: `export (default)? (function|const|class|let|var) name`
fn parse_js_export(line: &str) -> Option<Export> {
    let after_export = line.strip_prefix("export ")?.trim();

    let (is_default, rest) = if let Some(stripped) = after_export.strip_prefix("default ") {
        (true, stripped.trim())
    } else {
        (false, after_export)
    };

    // Match: function|const|class|let|var followed by a name
    let keywords = ["function ", "const ", "class ", "let ", "var "];
    for kw in &keywords {
        if let Some(after_kw) = rest.strip_prefix(kw) {
            let after_kw = after_kw.trim();
            // Extract the identifier (word characters)
            let name: String = after_kw
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                .collect();
            if !name.is_empty() {
                return Some(Export {
                    name,
                    kind: if is_default {
                        ExportKind::Default
                    } else {
                        ExportKind::Named
                    },
                });
            }
        }
    }

    None
}

/// Parse Rust function: `(pub)? fn name(`
fn parse_rust_fn(line: &str, line_no: u32) -> Option<FunctionSignature> {
    // Find `fn ` in the line
    let fn_idx = find_fn_keyword(line)?;
    let after_fn = &line[fn_idx + 3..]; // skip "fn "

    // Extract function name
    let name: String = after_fn
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();

    if name.is_empty() {
        return None;
    }

    // Check there's a `(` after the name (possibly with generics in between)
    let after_name = &after_fn[name.len()..];
    if !after_name.contains('(') {
        return None;
    }

    Some(FunctionSignature {
        name,
        params: Vec::new(),
        return_type: None,
        line: line_no,
    })
}

/// Find `fn ` keyword in a line, ensuring it appears as a keyword
/// (preceded by start-of-string, whitespace, `pub `, etc.)
fn find_fn_keyword(line: &str) -> Option<usize> {
    let trimmed = line.trim();
    // Patterns: "fn ", "pub fn ", "pub(crate) fn ", "pub(super) fn ", "async fn ", etc.
    if trimmed.starts_with("fn ") {
        let offset = line.len() - trimmed.len();
        return Some(offset);
    }

    // Look for " fn " preceded by a keyword
    let mut search_from = 0;
    while let Some(idx) = line[search_from..].find("fn ") {
        let abs_idx = search_from + idx;
        if abs_idx > 0 {
            let before = line.as_bytes()[abs_idx - 1];
            if before == b' ' || before == b'\t' || before == b')' {
                return Some(abs_idx);
            }
        }
        search_from = abs_idx + 1;
    }

    None
}

/// Parse JS function: `(async)? function name(`
fn parse_js_function(line: &str, line_no: u32) -> Option<FunctionSignature> {
    let trimmed = line.trim();

    let rest = if let Some(after) = trimmed.strip_prefix("async ") {
        after.trim()
    } else {
        trimmed
    };

    let after_function = rest.strip_prefix("function ")?.trim();

    // Handle function* (generators)
    let after_function = after_function.strip_prefix('*').unwrap_or(after_function);
    let after_function = after_function.trim();

    let name: String = after_function
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
        .collect();

    if name.is_empty() {
        return None;
    }

    let after_name = &after_function[name.len()..].trim_start();
    if !after_name.starts_with('(') {
        return None;
    }

    Some(FunctionSignature {
        name,
        params: Vec::new(),
        return_type: None,
        line: line_no,
    })
}

/// Parse Python def: `def name(`
fn parse_python_def(line: &str, line_no: u32) -> Option<FunctionSignature> {
    let trimmed = line.trim();

    // Also handle `async def`
    let rest = if let Some(after) = trimmed.strip_prefix("async ") {
        after.trim()
    } else {
        trimmed
    };

    let after_def = rest.strip_prefix("def ")?.trim();

    let name: String = after_def
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();

    if name.is_empty() {
        return None;
    }

    let after_name = &after_def[name.len()..].trim_start();
    if !after_name.starts_with('(') {
        return None;
    }

    Some(FunctionSignature {
        name,
        params: Vec::new(),
        return_type: None,
        line: line_no,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parser::LanguageParser;

    #[test]
    fn parses_esm_import() {
        let source = r#"import { useState, useEffect } from "react";"#;
        let imports = GenericParser.parse_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "react");
        assert_eq!(imports[0].names, vec!["useState", "useEffect"]);
    }

    #[test]
    fn parses_esm_default_import() {
        let source = r#"import React from "react";"#;
        let imports = GenericParser.parse_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "react");
        assert_eq!(imports[0].default, Some("React".to_string()));
    }

    #[test]
    fn parses_esm_single_quote() {
        let source = "import { foo } from 'bar';";
        let imports = GenericParser.parse_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "bar");
    }

    #[test]
    fn parses_commonjs_require() {
        let source = r#"const fs = require("fs");"#;
        let imports = GenericParser.parse_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "fs");
    }

    #[test]
    fn parses_commonjs_require_single_quote() {
        let source = "const path = require('path');";
        let imports = GenericParser.parse_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "path");
    }

    #[test]
    fn parses_python_from_import() {
        let source = "from pathlib import Path";
        let imports = GenericParser.parse_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "pathlib");
        assert_eq!(imports[0].names, vec!["Path"]);
    }

    #[test]
    fn parses_python_from_import_multiple() {
        let source = "from os.path import join, dirname, exists";
        let imports = GenericParser.parse_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "os.path");
        assert_eq!(imports[0].names, vec!["join", "dirname", "exists"]);
    }

    #[test]
    fn parses_rust_use() {
        let source = "use std::collections::HashMap;";
        let imports = GenericParser.parse_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "std::collections::HashMap");
    }

    #[test]
    fn parses_c_include_quotes() {
        let source = r#"#include "myheader.h""#;
        let imports = GenericParser.parse_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "myheader.h");
    }

    #[test]
    fn parses_c_include_angle() {
        let source = "#include <stdio.h>";
        let imports = GenericParser.parse_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "stdio.h");
    }

    #[test]
    fn parses_multiple_imports() {
        let source = r#"
import { useState } from "react";
const fs = require("fs");
from pathlib import Path
use std::io;
#include <stdlib.h>
"#;
        let imports = GenericParser.parse_imports(source);
        assert_eq!(imports.len(), 5);
        assert_eq!(imports[0].source, "react");
        assert_eq!(imports[1].source, "fs");
        assert_eq!(imports[2].source, "pathlib");
        assert_eq!(imports[3].source, "std::io");
        assert_eq!(imports[4].source, "stdlib.h");
    }

    #[test]
    fn parses_export_named() {
        let source = "export const myFunction = () => {};";
        let exports = GenericParser.parse_exports(source);
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].name, "myFunction");
        assert_eq!(exports[0].kind, ExportKind::Named);
    }

    #[test]
    fn parses_export_default_function() {
        let source = "export default function App() {}";
        let exports = GenericParser.parse_exports(source);
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].name, "App");
        assert_eq!(exports[0].kind, ExportKind::Default);
    }

    #[test]
    fn parses_export_class() {
        let source = "export class MyService {}";
        let exports = GenericParser.parse_exports(source);
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].name, "MyService");
        assert_eq!(exports[0].kind, ExportKind::Named);
    }

    #[test]
    fn parses_export_let_and_var() {
        let source = "export let counter = 0;\nexport var legacy = true;";
        let exports = GenericParser.parse_exports(source);
        assert_eq!(exports.len(), 2);
        assert_eq!(exports[0].name, "counter");
        assert_eq!(exports[1].name, "legacy");
    }

    #[test]
    fn parses_rust_fn() {
        let source = "fn main() {\n    println!(\"hello\");\n}";
        let functions = GenericParser.parse_functions(source);
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name, "main");
        assert_eq!(functions[0].line, 1);
    }

    #[test]
    fn parses_rust_pub_fn() {
        let source = "pub fn new(x: i32) -> Self {";
        let functions = GenericParser.parse_functions(source);
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name, "new");
    }

    #[test]
    fn parses_rust_pub_crate_fn() {
        let source = "pub(crate) fn helper() {";
        let functions = GenericParser.parse_functions(source);
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name, "helper");
    }

    #[test]
    fn parses_js_function() {
        let source = "function greet(name) { return `Hello, ${name}`; }";
        let functions = GenericParser.parse_functions(source);
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name, "greet");
    }

    #[test]
    fn parses_async_function() {
        let source = "async function fetchData() {}";
        let functions = GenericParser.parse_functions(source);
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name, "fetchData");
    }

    #[test]
    fn parses_python_def() {
        let source = "def calculate(a, b):\n    return a + b";
        let functions = GenericParser.parse_functions(source);
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name, "calculate");
        assert_eq!(functions[0].line, 1);
    }

    #[test]
    fn parses_async_python_def() {
        let source = "async def handler(request):";
        let functions = GenericParser.parse_functions(source);
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name, "handler");
    }

    #[test]
    fn parses_multiple_functions() {
        let source = r#"
fn rust_func() {}
function jsFunc() {}
def py_func():
    pass
"#;
        let functions = GenericParser.parse_functions(source);
        assert_eq!(functions.len(), 3);
        assert_eq!(functions[0].name, "rust_func");
        assert_eq!(functions[1].name, "jsFunc");
        assert_eq!(functions[2].name, "py_func");
    }

    #[test]
    fn empty_source_returns_empty() {
        assert!(GenericParser.parse_imports("").is_empty());
        assert!(GenericParser.parse_exports("").is_empty());
        assert!(GenericParser.parse_functions("").is_empty());
    }

    #[test]
    fn binary_gibberish_no_panic() {
        let source = "\x00\x01\x02\x7f garbage \t\n\r\0";
        let _ = GenericParser.parse_imports(source);
        let _ = GenericParser.parse_exports(source);
        let _ = GenericParser.parse_functions(source);
    }

    #[test]
    fn generic_parser_language_is_unknown() {
        assert_eq!(GenericParser.language(), Language::Unknown);
    }

    #[test]
    fn generic_parser_can_parse_anything() {
        assert!(GenericParser.can_parse("go"));
        assert!(GenericParser.can_parse("rb"));
        assert!(GenericParser.can_parse("xyz"));
    }

    #[test]
    fn non_import_lines_ignored() {
        let source = "let x = 5;\nconst y = 10;\nprintln!(\"hello\");";
        assert!(GenericParser.parse_imports(source).is_empty());
    }

    #[test]
    fn non_export_lines_ignored() {
        let source = "const x = 5;\nfunction foo() {}";
        assert!(GenericParser.parse_exports(source).is_empty());
    }

    #[test]
    fn function_line_numbers_correct() {
        let source = "// comment\n\ndef foo():\n    pass\n\ndef bar():\n    pass";
        let functions = GenericParser.parse_functions(source);
        assert_eq!(functions.len(), 2);
        assert_eq!(functions[0].name, "foo");
        assert_eq!(functions[0].line, 3);
        assert_eq!(functions[1].name, "bar");
        assert_eq!(functions[1].line, 6);
    }
}
