use tree_sitter::{Node, Parser};

use crate::core::types::{Export, ExportKind, FunctionSignature, Import, Language};

use super::LanguageParser;

/// Tree-sitter based parser for TypeScript and JavaScript files.
pub struct TypeScriptParser;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a tree-sitter parser configured for TypeScript.
fn make_parser() -> Parser {
    let mut parser = Parser::new();
    let language: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    parser
        .set_language(&language)
        .expect("failed to set TypeScript language");
    parser
}

/// Extract the text of a node from source, returning an empty string on failure.
fn node_text<'a>(node: Node<'a>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

/// Strip surrounding quotes from a string literal.
fn strip_quotes(s: &str) -> String {
    s.trim_matches('"')
        .trim_matches('\'')
        .trim_matches('`')
        .to_string()
}

/// Iterate over the named children of a node.
fn named_children(node: Node) -> Vec<Node> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

/// Iterate over all children (named + anonymous) of a node.
fn all_children(node: Node) -> Vec<Node> {
    let mut cursor = node.walk();
    node.children(&mut cursor).collect()
}

// ---------------------------------------------------------------------------
// Import extraction
// ---------------------------------------------------------------------------

/// Walk an `import_clause` node to fill `default` and `names`.
fn extract_import_clause(clause: Node, source: &str, import: &mut Import) {
    for child in all_children(clause) {
        match child.kind() {
            "identifier" => {
                // Default import
                import.default = Some(node_text(child, source).to_string());
            }
            "named_imports" => {
                for spec in named_children(child) {
                    if spec.kind() == "import_specifier" {
                        // Use the alias if present, otherwise the name.
                        let name_node = spec
                            .child_by_field_name("alias")
                            .or_else(|| spec.child_by_field_name("name"));
                        if let Some(n) = name_node {
                            import.names.push(node_text(n, source).to_string());
                        }
                    }
                }
            }
            "namespace_import" => {
                // import * as Foo from "..."
                if let Some(id) = child.named_child(0) {
                    import.default = Some(format!("* as {}", node_text(id, source)));
                }
            }
            _ => {}
        }
    }
}

fn walk_imports(node: Node, source: &str, imports: &mut Vec<Import>) {
    match node.kind() {
        "import_statement" => {
            let mut import = Import {
                source: String::new(),
                names: vec![],
                default: None,
                resolved_path: None,
            };

            for child in all_children(node) {
                match child.kind() {
                    "string" => {
                        import.source = strip_quotes(node_text(child, source));
                    }
                    "import_clause" => {
                        extract_import_clause(child, source, &mut import);
                    }
                    _ => {}
                }
            }

            if !import.source.is_empty() {
                imports.push(import);
            }
        }
        "call_expression" => {
            // require("...") or dynamic import("...")
            if let Some(func) = node.child_by_field_name("function") {
                let func_text = node_text(func, source);
                if func_text == "require" {
                    if let Some(args) = node.child_by_field_name("arguments") {
                        // First named child of the arguments node is the string
                        for arg in named_children(args) {
                            if arg.kind() == "string" {
                                imports.push(Import {
                                    source: strip_quotes(node_text(arg, source)),
                                    names: vec![],
                                    default: None,
                                    resolved_path: None,
                                });
                                break;
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }

    // Recurse into children
    for child in named_children(node) {
        walk_imports(child, source, imports);
    }
}

// ---------------------------------------------------------------------------
// Export extraction
// ---------------------------------------------------------------------------

fn walk_exports(node: Node, source: &str, exports: &mut Vec<Export>) {
    if node.kind() == "export_statement" {
        // Check for re-export: export { ... } from "..."
        let re_export_source = node
            .child_by_field_name("source")
            .map(|s| strip_quotes(node_text(s, source)));

        // Check for `default` keyword
        let is_default = all_children(node)
            .iter()
            .any(|c| c.kind() == "default" || node_text(*c, source) == "default");

        // Check for declaration child
        if let Some(decl) = node.child_by_field_name("declaration") {
            match decl.kind() {
                "function_declaration" | "generator_function_declaration" => {
                    let name = decl
                        .child_by_field_name("name")
                        .map(|n| node_text(n, source).to_string())
                        .unwrap_or_else(|| "default".to_string());
                    let kind = if is_default {
                        ExportKind::Default
                    } else {
                        ExportKind::Named
                    };
                    exports.push(Export { name, kind });
                }
                "class_declaration" => {
                    let name = decl
                        .child_by_field_name("name")
                        .map(|n| node_text(n, source).to_string())
                        .unwrap_or_else(|| "default".to_string());
                    let kind = if is_default {
                        ExportKind::Default
                    } else {
                        ExportKind::Named
                    };
                    exports.push(Export { name, kind });
                }
                "lexical_declaration" | "variable_declaration" => {
                    // export const Foo = ..., Bar = ...
                    for child in named_children(decl) {
                        if child.kind() == "variable_declarator" {
                            if let Some(name_node) = child.child_by_field_name("name") {
                                exports.push(Export {
                                    name: node_text(name_node, source).to_string(),
                                    kind: ExportKind::Named,
                                });
                            }
                        }
                    }
                }
                "type_alias_declaration" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        exports.push(Export {
                            name: node_text(name_node, source).to_string(),
                            kind: ExportKind::Named,
                        });
                    }
                }
                "interface_declaration" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        exports.push(Export {
                            name: node_text(name_node, source).to_string(),
                            kind: ExportKind::Named,
                        });
                    }
                }
                "enum_declaration" => {
                    if let Some(name_node) = decl.child_by_field_name("name") {
                        exports.push(Export {
                            name: node_text(name_node, source).to_string(),
                            kind: ExportKind::Named,
                        });
                    }
                }
                _ => {
                    // e.g. export default <expression>
                    if is_default {
                        exports.push(Export {
                            name: "default".to_string(),
                            kind: ExportKind::Default,
                        });
                    }
                }
            }
        } else if let Some(ref re_src) = re_export_source {
            // Re-export: export { ... } from "..."
            let mut found_names = false;
            for child in named_children(node) {
                if child.kind() == "export_clause" {
                    for spec in named_children(child) {
                        if spec.kind() == "export_specifier" {
                            let name = spec
                                .child_by_field_name("alias")
                                .or_else(|| spec.child_by_field_name("name"))
                                .map(|n| node_text(n, source).to_string())
                                .unwrap_or_default();
                            exports.push(Export {
                                name,
                                kind: ExportKind::ReExport {
                                    source: re_src.clone(),
                                },
                            });
                            found_names = true;
                        }
                    }
                }
            }
            if !found_names {
                // export * from "..."
                exports.push(Export {
                    name: "*".to_string(),
                    kind: ExportKind::ReExport {
                        source: re_src.clone(),
                    },
                });
            }
        } else if is_default {
            // export default <expression> (no declaration field)
            exports.push(Export {
                name: "default".to_string(),
                kind: ExportKind::Default,
            });
        } else {
            // export { Foo, Bar }  (local re-exports without source)
            for child in named_children(node) {
                if child.kind() == "export_clause" {
                    for spec in named_children(child) {
                        if spec.kind() == "export_specifier" {
                            let name = spec
                                .child_by_field_name("alias")
                                .or_else(|| spec.child_by_field_name("name"))
                                .map(|n| node_text(n, source).to_string())
                                .unwrap_or_default();
                            exports.push(Export {
                                name,
                                kind: ExportKind::Named,
                            });
                        }
                    }
                }
            }
        }
    }

    // Recurse
    for child in named_children(node) {
        walk_exports(child, source, exports);
    }
}

// ---------------------------------------------------------------------------
// Function extraction
// ---------------------------------------------------------------------------

/// Extract parameter names from a `formal_parameters` node.
fn extract_params(params_node: Node, source: &str) -> Vec<String> {
    let mut params = Vec::new();
    for child in named_children(params_node) {
        match child.kind() {
            "required_parameter" | "optional_parameter" => {
                if let Some(pattern) = child.child_by_field_name("pattern") {
                    params.push(node_text(pattern, source).to_string());
                }
            }
            "rest_pattern" => {
                // ...args
                params.push(node_text(child, source).to_string());
            }
            "identifier" => {
                params.push(node_text(child, source).to_string());
            }
            _ => {
                // Fallback: just use the text
                params.push(node_text(child, source).to_string());
            }
        }
    }
    params
}

/// Extract the return type annotation text, if present.
fn extract_return_type(node: Node, source: &str) -> Option<String> {
    node.child_by_field_name("return_type")
        .map(|rt| {
            // The return_type field includes the `: ` prefix via a type_annotation node.
            let text = node_text(rt, source).to_string();
            text.trim_start_matches(':').trim().to_string()
        })
        .filter(|s| !s.is_empty())
}

fn walk_functions(node: Node, source: &str, functions: &mut Vec<FunctionSignature>) {
    match node.kind() {
        "function_declaration" | "generator_function_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = node_text(name_node, source).to_string();
                let params = node
                    .child_by_field_name("parameters")
                    .map(|p| extract_params(p, source))
                    .unwrap_or_default();
                let return_type = extract_return_type(node, source);
                let line = node.start_position().row as u32 + 1;
                functions.push(FunctionSignature {
                    name,
                    params,
                    return_type,
                    line,
                });
            }
        }
        "lexical_declaration" | "variable_declaration" => {
            // const foo = (...) => { ... }
            // const foo = function(...) { ... }
            for child in named_children(node) {
                if child.kind() == "variable_declarator" {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        if let Some(value) = child.child_by_field_name("value") {
                            if value.kind() == "arrow_function"
                                || value.kind() == "function_expression"
                                || value.kind() == "generator_function"
                            {
                                let name = node_text(name_node, source).to_string();
                                let params = value
                                    .child_by_field_name("parameters")
                                    .map(|p| extract_params(p, source))
                                    .unwrap_or_default();
                                let return_type = extract_return_type(value, source);
                                let line = node.start_position().row as u32 + 1;
                                functions.push(FunctionSignature {
                                    name,
                                    params,
                                    return_type,
                                    line,
                                });
                            }
                        }
                    }
                }
            }
        }
        "export_statement" => {
            // export function foo() {}  /  export const foo = () => {}
            // Check if there's a declaration child and recurse into it specifically
            if let Some(decl) = node.child_by_field_name("declaration") {
                walk_functions(decl, source, functions);
            }
            // Don't recurse further — we handled the declaration
            return;
        }
        _ => {}
    }

    // Recurse into children
    for child in named_children(node) {
        walk_functions(child, source, functions);
    }
}

// ---------------------------------------------------------------------------
// LanguageParser implementation
// ---------------------------------------------------------------------------

impl LanguageParser for TypeScriptParser {
    fn language(&self) -> Language {
        Language::TypeScript
    }

    fn can_parse(&self, extension: &str) -> bool {
        matches!(extension, "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs")
    }

    fn parse_imports(&self, source: &str) -> Vec<Import> {
        if source.is_empty() {
            return Vec::new();
        }
        let mut parser = make_parser();
        let Some(tree) = parser.parse(source, None) else {
            return Vec::new();
        };
        let mut imports = Vec::new();
        walk_imports(tree.root_node(), source, &mut imports);
        imports
    }

    fn parse_exports(&self, source: &str) -> Vec<Export> {
        if source.is_empty() {
            return Vec::new();
        }
        let mut parser = make_parser();
        let Some(tree) = parser.parse(source, None) else {
            return Vec::new();
        };
        let mut exports = Vec::new();
        walk_exports(tree.root_node(), source, &mut exports);
        exports
    }

    fn parse_functions(&self, source: &str) -> Vec<FunctionSignature> {
        if source.is_empty() {
            return Vec::new();
        }
        let mut parser = make_parser();
        let Some(tree) = parser.parse(source, None) else {
            return Vec::new();
        };
        let mut functions = Vec::new();
        walk_functions(tree.root_node(), source, &mut functions);
        functions
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parser::LanguageParser;

    #[test]
    fn esm_named_imports() {
        let source = r#"import { useState, useEffect } from "react";"#;
        let imports = TypeScriptParser.parse_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "react");
        assert!(imports[0].names.contains(&"useState".to_string()));
        assert!(imports[0].names.contains(&"useEffect".to_string()));
    }

    #[test]
    fn esm_default_import() {
        let source = r#"import React from "react";"#;
        let imports = TypeScriptParser.parse_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "react");
        assert_eq!(imports[0].default, Some("React".to_string()));
    }

    #[test]
    fn side_effect_import() {
        let source = r#"import "./styles.css";"#;
        let imports = TypeScriptParser.parse_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "./styles.css");
    }

    #[test]
    fn commonjs_require() {
        let source = r#"const fs = require("fs");"#;
        let imports = TypeScriptParser.parse_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "fs");
    }

    #[test]
    fn named_export() {
        let source = "export const API_URL = 'http://example.com';";
        let exports = TypeScriptParser.parse_exports(source);
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].name, "API_URL");
    }

    #[test]
    fn default_export_function() {
        let source = "export default function App() {}";
        let exports = TypeScriptParser.parse_exports(source);
        assert!(!exports.is_empty());
        assert!(exports.iter().any(|e| e.kind == ExportKind::Default));
    }

    #[test]
    fn function_declaration() {
        let source = "function greet(name: string): void {}";
        let fns = TypeScriptParser.parse_functions(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "greet");
    }

    #[test]
    fn empty_source() {
        assert!(TypeScriptParser.parse_imports("").is_empty());
        assert!(TypeScriptParser.parse_exports("").is_empty());
        assert!(TypeScriptParser.parse_functions("").is_empty());
    }

    #[test]
    fn multiple_imports() {
        let source = r#"
import React from "react";
import { render } from "react-dom";
import "./styles.css";
"#;
        let imports = TypeScriptParser.parse_imports(source);
        assert_eq!(imports.len(), 3);
    }

    #[test]
    fn default_and_named_import() {
        let source = r#"import React, { useState, useEffect } from "react";"#;
        let imports = TypeScriptParser.parse_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "react");
        assert_eq!(imports[0].default, Some("React".to_string()));
        assert!(imports[0].names.contains(&"useState".to_string()));
        assert!(imports[0].names.contains(&"useEffect".to_string()));
    }

    #[test]
    fn namespace_import() {
        let source = r#"import * as path from "path";"#;
        let imports = TypeScriptParser.parse_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "path");
        assert_eq!(imports[0].default, Some("* as path".to_string()));
    }

    #[test]
    fn re_export() {
        let source = r#"export { foo, bar } from "./utils";"#;
        let exports = TypeScriptParser.parse_exports(source);
        assert_eq!(exports.len(), 2);
        assert!(exports.iter().any(|e| e.name == "foo"
            && e.kind
                == ExportKind::ReExport {
                    source: "./utils".to_string()
                }));
    }

    #[test]
    fn export_default_expression() {
        let source = "export default 42;";
        let exports = TypeScriptParser.parse_exports(source);
        assert!(!exports.is_empty());
        assert!(exports.iter().any(|e| e.kind == ExportKind::Default));
    }

    #[test]
    fn arrow_function_const() {
        let source = "const greet = (name: string): void => { console.log(name); };";
        let fns = TypeScriptParser.parse_functions(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "greet");
    }

    #[test]
    fn exported_function_counted_as_function() {
        let source = "export function handleClick(event: Event): void {}";
        let fns = TypeScriptParser.parse_functions(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "handleClick");
    }

    #[test]
    fn function_with_params_extracted() {
        let source = "function add(a: number, b: number): number { return a + b; }";
        let fns = TypeScriptParser.parse_functions(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "add");
        assert_eq!(fns[0].params.len(), 2);
    }

    #[test]
    fn malformed_source_no_panic() {
        let source = "import { from \n export cons = ;; }{";
        // Should not panic, just return partial or empty results
        let _ = TypeScriptParser.parse_imports(source);
        let _ = TypeScriptParser.parse_exports(source);
        let _ = TypeScriptParser.parse_functions(source);
    }

    #[test]
    fn export_class() {
        let source = "export class MyService {}";
        let exports = TypeScriptParser.parse_exports(source);
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].name, "MyService");
        assert_eq!(exports[0].kind, ExportKind::Named);
    }

    #[test]
    fn export_star_re_export() {
        let source = r#"export * from "./module";"#;
        let exports = TypeScriptParser.parse_exports(source);
        assert!(!exports.is_empty());
        assert!(exports
            .iter()
            .any(|e| matches!(&e.kind, ExportKind::ReExport { source } if source == "./module")));
    }
}
