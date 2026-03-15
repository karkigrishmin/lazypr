use crate::core::types::{Export, ExportKind, FunctionSignature, Import, Language};

use super::LanguageParser;

/// Regex-based parser for Rust files.
pub struct RustParser;

impl LanguageParser for RustParser {
    fn language(&self) -> Language {
        Language::Rust
    }

    fn can_parse(&self, extension: &str) -> bool {
        extension == "rs"
    }

    fn parse_imports(&self, source: &str) -> Vec<Import> {
        let mut imports = Vec::new();

        for line in source.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("use ") {
                // use std::collections::HashMap;
                // use crate::core::{Import, Export};
                // use super::LanguageParser;
                let rest = trimmed
                    .strip_prefix("use ")
                    .unwrap_or("")
                    .trim_end_matches(';')
                    .trim();

                if rest.is_empty() {
                    continue;
                }

                if let Some(brace_start) = rest.find('{') {
                    // use crate::core::{Import, Export};
                    let source_path = rest[..brace_start].trim_end_matches("::").trim();
                    let names_part = &rest[brace_start + 1..];
                    let names_part = names_part.trim_end_matches('}').trim();

                    let names: Vec<String> = names_part
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();

                    imports.push(Import {
                        source: source_path.to_string(),
                        names,
                        default: None,
                        resolved_path: None,
                    });
                } else {
                    // use std::collections::HashMap;
                    // Could have `as` alias: use std::io::Result as IoResult;
                    let path = rest.split(" as ").next().unwrap_or(rest).trim();
                    imports.push(Import {
                        source: path.to_string(),
                        names: Vec::new(),
                        default: None,
                        resolved_path: None,
                    });
                }
            } else if trimmed.starts_with("mod ") && trimmed.ends_with(';') {
                // mod parser; — module declaration (not inline mod block)
                let rest = trimmed
                    .strip_prefix("mod ")
                    .unwrap_or("")
                    .trim_end_matches(';')
                    .trim();
                if !rest.is_empty() {
                    imports.push(Import {
                        source: rest.to_string(),
                        names: Vec::new(),
                        default: None,
                        resolved_path: None,
                    });
                }
            } else if trimmed.starts_with("pub mod ") && trimmed.ends_with(';') {
                let rest = trimmed
                    .strip_prefix("pub mod ")
                    .unwrap_or("")
                    .trim_end_matches(';')
                    .trim();
                if !rest.is_empty() {
                    imports.push(Import {
                        source: rest.to_string(),
                        names: Vec::new(),
                        default: None,
                        resolved_path: None,
                    });
                }
            }
        }

        imports
    }

    fn parse_exports(&self, source: &str) -> Vec<Export> {
        let mut exports = Vec::new();

        for line in source.lines() {
            let trimmed = line.trim();

            // pub struct MyStruct
            // pub enum MyEnum
            // pub trait MyTrait
            // pub type Alias
            let pub_prefixes = [
                "pub struct ",
                "pub enum ",
                "pub trait ",
                "pub type ",
                "pub(crate) struct ",
                "pub(crate) enum ",
                "pub(crate) trait ",
                "pub(crate) type ",
            ];

            for prefix in &pub_prefixes {
                if let Some(rest) = trimmed.strip_prefix(prefix) {
                    let name = rest
                        .split(|c: char| {
                            c == '<'
                                || c == '('
                                || c == '{'
                                || c == ';'
                                || c == ':'
                                || c.is_whitespace()
                        })
                        .next()
                        .unwrap_or("")
                        .trim();
                    if !name.is_empty() {
                        exports.push(Export {
                            name: name.to_string(),
                            kind: ExportKind::Named,
                        });
                    }
                    break;
                }
            }
        }

        exports
    }

    fn parse_functions(&self, source: &str) -> Vec<FunctionSignature> {
        let mut functions = Vec::new();

        for (line_no, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            // Match: fn, pub fn, pub async fn, async fn, pub(crate) fn, etc.
            let fn_rest = extract_fn_rest(trimmed);

            if let Some(rest) = fn_rest {
                // rest starts right after "fn "
                // Extract function name
                let name = rest
                    .split(['(', '<'])
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if name.is_empty() {
                    continue;
                }

                // Extract params between ( and matching )
                let params = extract_params(rest);

                // Extract return type after ->
                let return_type = extract_return_type(rest);

                functions.push(FunctionSignature {
                    name,
                    params,
                    return_type,
                    line: (line_no + 1) as u32,
                });
            }
        }

        functions
    }
}

/// Try to extract the portion after "fn " from various fn signatures.
fn extract_fn_rest(trimmed: &str) -> Option<&str> {
    // Try patterns in order of specificity
    let patterns = [
        "pub async fn ",
        "pub(crate) async fn ",
        "pub(crate) fn ",
        "pub const fn ",
        "pub unsafe fn ",
        "pub fn ",
        "async fn ",
        "const fn ",
        "unsafe fn ",
    ];

    for pattern in &patterns {
        if let Some(rest) = trimmed.strip_prefix(pattern) {
            return Some(rest);
        }
    }

    // Plain "fn " — but not "fn_" or other words containing "fn"
    if let Some(rest) = trimmed.strip_prefix("fn ") {
        return Some(rest);
    }

    None
}

/// Extract parameter names from the text after a function name.
fn extract_params(rest: &str) -> Vec<String> {
    let open = match rest.find('(') {
        Some(p) => p,
        None => return Vec::new(),
    };

    // Find matching close paren (handling nested parens)
    let after_open = &rest[open + 1..];
    let mut depth = 1;
    let mut close_offset = after_open.len();
    for (i, ch) in after_open.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close_offset = i;
                    break;
                }
            }
            _ => {}
        }
    }

    let params_str = &after_open[..close_offset];
    if params_str.trim().is_empty() {
        return Vec::new();
    }

    // Split by commas at depth 0 (to handle generic params like Vec<A, B>)
    let param_parts = split_top_level_commas(params_str);

    param_parts
        .into_iter()
        .map(|p| {
            let p = p.trim();
            // For Rust params like `source: &str`, take the name before ':'
            // But skip `self`, `&self`, `&mut self`, `mut self`
            let stripped = p.trim_start_matches('&').trim_start_matches("mut ").trim();
            if stripped == "self" {
                return String::new();
            }
            // mut source: &str -> source
            let p = p.trim_start_matches("mut ");
            p.split(':').next().unwrap_or(p).trim().to_string()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// Split a string by commas, but only at the top level (not inside <>, (), []).
fn split_top_level_commas(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut start = 0;

    for (i, ch) in s.char_indices() {
        match ch {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            ',' if depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

/// Extract the return type from a function signature line.
fn extract_return_type(rest: &str) -> Option<String> {
    // Find the closing paren first, then look for ->
    let open = rest.find('(')?;
    let after_open = &rest[open + 1..];
    let mut depth = 1;
    let mut close_offset = after_open.len();
    for (i, ch) in after_open.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close_offset = i;
                    break;
                }
            }
            _ => {}
        }
    }

    let after_params = &after_open[close_offset + 1..];
    let arrow_pos = after_params.find("->")?;
    let after_arrow = after_params[arrow_pos + 2..].trim();

    // Return type ends at `{`, `where`, or end of string
    let rt = after_arrow.split('{').next().unwrap_or(after_arrow).trim();

    // Also strip `where` clauses
    let rt = rt.split("where").next().unwrap_or(rt).trim();

    if rt.is_empty() {
        None
    } else {
        Some(rt.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parser() -> RustParser {
        RustParser
    }

    #[test]
    fn use_std_hashmap() {
        let imports = parser().parse_imports("use std::collections::HashMap;");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "std::collections::HashMap");
        assert!(imports[0].names.is_empty());
    }

    #[test]
    fn use_crate_with_braces() {
        let imports = parser().parse_imports("use crate::core::{Import, Export};");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "crate::core");
        assert_eq!(imports[0].names, vec!["Import", "Export"]);
    }

    #[test]
    fn use_super() {
        let imports = parser().parse_imports("use super::LanguageParser;");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "super::LanguageParser");
    }

    #[test]
    fn mod_declaration() {
        let imports = parser().parse_imports("mod parser;");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "parser");
    }

    #[test]
    fn pub_mod_declaration() {
        let imports = parser().parse_imports("pub mod utils;");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "utils");
    }

    #[test]
    fn mod_inline_block_ignored() {
        // mod foo { ... } is an inline module, not a declaration — no semicolon
        let imports = parser().parse_imports("mod foo {");
        assert!(imports.is_empty());
    }

    #[test]
    fn pub_fn_signature() {
        let source = "pub fn parse(source: &str) -> Vec<Import> {";
        let fns = parser().parse_functions(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "parse");
        assert_eq!(fns[0].params, vec!["source"]);
        assert_eq!(fns[0].return_type.as_deref(), Some("Vec<Import>"));
        assert_eq!(fns[0].line, 1);
    }

    #[test]
    fn private_fn() {
        let source = "fn helper() -> bool {";
        let fns = parser().parse_functions(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "helper");
        assert!(fns[0].params.is_empty());
        assert_eq!(fns[0].return_type.as_deref(), Some("bool"));
    }

    #[test]
    fn pub_async_fn() {
        let source = "pub async fn fetch(url: &str) -> Result<()> {";
        let fns = parser().parse_functions(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "fetch");
        assert_eq!(fns[0].params, vec!["url"]);
        assert_eq!(fns[0].return_type.as_deref(), Some("Result<()>"));
    }

    #[test]
    fn fn_with_self() {
        let source = "    fn language(&self) -> Language {";
        let fns = parser().parse_functions(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "language");
        assert!(fns[0].params.is_empty());
        assert_eq!(fns[0].return_type.as_deref(), Some("Language"));
    }

    #[test]
    fn fn_no_return_type() {
        let source = "fn do_stuff(x: i32) {";
        let fns = parser().parse_functions(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "do_stuff");
        assert_eq!(fns[0].params, vec!["x"]);
        assert!(fns[0].return_type.is_none());
    }

    #[test]
    fn pub_struct_export() {
        let source = "pub struct MyStruct {";
        let exports = parser().parse_exports(source);
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].name, "MyStruct");
        assert_eq!(exports[0].kind, ExportKind::Named);
    }

    #[test]
    fn pub_enum_export() {
        let source = "pub enum FileStatus {";
        let exports = parser().parse_exports(source);
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].name, "FileStatus");
    }

    #[test]
    fn pub_trait_export() {
        let source = "pub trait LanguageParser: Send + Sync {";
        let exports = parser().parse_exports(source);
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].name, "LanguageParser");
    }

    #[test]
    fn pub_type_export() {
        let source = "pub type Result<T> = std::result::Result<T, Error>;";
        let exports = parser().parse_exports(source);
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].name, "Result");
    }

    #[test]
    fn private_struct_not_exported() {
        let source = "struct InternalHelper {";
        let exports = parser().parse_exports(source);
        assert!(exports.is_empty());
    }

    #[test]
    fn empty_source() {
        let p = parser();
        assert!(p.parse_imports("").is_empty());
        assert!(p.parse_exports("").is_empty());
        assert!(p.parse_functions("").is_empty());
    }

    #[test]
    fn multiple_imports() {
        let source =
            "use std::io;\nuse std::collections::HashMap;\nmod parser;\nuse super::LanguageParser;";
        let imports = parser().parse_imports(source);
        assert_eq!(imports.len(), 4);
        assert_eq!(imports[0].source, "std::io");
        assert_eq!(imports[1].source, "std::collections::HashMap");
        assert_eq!(imports[2].source, "parser");
        assert_eq!(imports[3].source, "super::LanguageParser");
    }

    #[test]
    fn pub_struct_with_generics() {
        let source = "pub struct Vec<T> {";
        let exports = parser().parse_exports(source);
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].name, "Vec");
    }
}
