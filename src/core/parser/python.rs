use crate::core::types::{Export, ExportKind, FunctionSignature, Import, Language};

use super::LanguageParser;

/// Regex-based parser for Python files.
pub struct PythonParser;

impl LanguageParser for PythonParser {
    fn language(&self) -> Language {
        Language::Python
    }

    fn can_parse(&self, extension: &str) -> bool {
        extension == "py"
    }

    fn parse_imports(&self, source: &str) -> Vec<Import> {
        let mut imports = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            if line.starts_with("from ") {
                // from X import Y  or  from X import (multiline)
                if let Some(rest) = line.strip_prefix("from ") {
                    if let Some(import_pos) = rest.find(" import ") {
                        let source_mod = rest[..import_pos].trim().to_string();
                        let names_part = rest[import_pos + 8..].trim();

                        let names_str = if names_part.starts_with('(') {
                            // multiline import: collect until closing paren
                            let mut collected = names_part
                                .strip_prefix('(')
                                .unwrap_or(names_part)
                                .to_string();
                            if !collected.contains(')') {
                                i += 1;
                                while i < lines.len() {
                                    let cont = lines[i].trim();
                                    if cont.contains(')') {
                                        let before_paren =
                                            cont.split(')').next().unwrap_or("").trim();
                                        if !before_paren.is_empty() {
                                            collected.push_str(", ");
                                            collected.push_str(before_paren);
                                        }
                                        break;
                                    }
                                    collected.push_str(", ");
                                    collected.push_str(cont.trim_end_matches(','));
                                    i += 1;
                                }
                            } else {
                                // Single-line parenthesized: from x import (A, B)
                                collected = collected.trim_end_matches(')').to_string();
                            }
                            collected
                        } else {
                            names_part.to_string()
                        };

                        let names: Vec<String> = names_str
                            .split(',')
                            .map(|s| s.trim().trim_end_matches(',').to_string())
                            .filter(|s| !s.is_empty())
                            .collect();

                        imports.push(Import {
                            source: source_mod,
                            names,
                            default: None,
                            resolved_path: None,
                        });
                    }
                }
            } else if line.starts_with("import ") {
                // import X  or  import X, Y
                if let Some(rest) = line.strip_prefix("import ") {
                    let modules: Vec<&str> = rest.split(',').map(|s| s.trim()).collect();
                    for module in modules {
                        if module.is_empty() {
                            continue;
                        }
                        // Handle `import os as operating_system` — use the base module
                        let mod_name = module.split(" as ").next().unwrap_or(module).trim();
                        imports.push(Import {
                            source: mod_name.to_string(),
                            names: Vec::new(),
                            default: None,
                            resolved_path: None,
                        });
                    }
                }
            }

            i += 1;
        }

        imports
    }

    fn parse_exports(&self, source: &str) -> Vec<Export> {
        let mut exports = Vec::new();

        for line in source.lines() {
            let trimmed = line.trim_start();

            // Only top-level definitions: no leading whitespace
            if line != trimmed {
                continue;
            }

            if let Some(rest) = trimmed.strip_prefix("class ") {
                // class ClassName(Base): or class ClassName:
                let name = rest
                    .split(|c: char| c == '(' || c == ':' || c.is_whitespace())
                    .next()
                    .unwrap_or("")
                    .trim();
                if !name.is_empty() {
                    exports.push(Export {
                        name: name.to_string(),
                        kind: ExportKind::Named,
                    });
                }
            } else if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                let rest = if let Some(r) = trimmed.strip_prefix("async def ") {
                    r
                } else {
                    trimmed.strip_prefix("def ").unwrap_or("")
                };
                let name = rest
                    .split(|c: char| c == '(' || c.is_whitespace())
                    .next()
                    .unwrap_or("")
                    .trim();
                if !name.is_empty() {
                    exports.push(Export {
                        name: name.to_string(),
                        kind: ExportKind::Named,
                    });
                }
            }
        }

        exports
    }

    fn parse_functions(&self, source: &str) -> Vec<FunctionSignature> {
        let mut functions = Vec::new();

        for (line_no, line) in source.lines().enumerate() {
            let trimmed = line.trim();

            let rest = if let Some(r) = trimmed.strip_prefix("async def ") {
                Some(r)
            } else {
                trimmed.strip_prefix("def ")
            };

            if let Some(rest) = rest {
                // Extract function name
                let name = rest.split('(').next().unwrap_or("").trim().to_string();
                if name.is_empty() {
                    continue;
                }

                // Extract params between first ( and matching )
                let after_open = if let Some(pos) = rest.find('(') {
                    &rest[pos + 1..]
                } else {
                    ""
                };

                // Find the closing paren (simple — doesn't handle nested parens deeply)
                let params_str = if let Some(close) = after_open.find(')') {
                    &after_open[..close]
                } else {
                    after_open
                };

                let params: Vec<String> = params_str
                    .split(',')
                    .map(|p| {
                        // Take only the parameter name (before : or =)
                        let p = p.trim();
                        p.split(':')
                            .next()
                            .unwrap_or(p)
                            .split('=')
                            .next()
                            .unwrap_or(p)
                            .trim()
                            .to_string()
                    })
                    .filter(|s| !s.is_empty() && s != "self" && s != "cls")
                    .collect();

                // Extract return type: -> Type:
                let return_type = if let Some(arrow_pos) = trimmed.find("->") {
                    let after_arrow = trimmed[arrow_pos + 2..].trim();
                    let rt = after_arrow.trim_end_matches(':').trim();
                    if rt.is_empty() {
                        None
                    } else {
                        Some(rt.to_string())
                    }
                } else {
                    None
                };

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

#[cfg(test)]
mod tests {
    use super::*;

    fn parser() -> PythonParser {
        PythonParser
    }

    #[test]
    fn import_os() {
        let imports = parser().parse_imports("import os");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "os");
        assert!(imports[0].names.is_empty());
    }

    #[test]
    fn import_multiple_modules() {
        let imports = parser().parse_imports("import os, sys");
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].source, "os");
        assert_eq!(imports[1].source, "sys");
    }

    #[test]
    fn from_pathlib_import_path() {
        let imports = parser().parse_imports("from pathlib import Path");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "pathlib");
        assert_eq!(imports[0].names, vec!["Path"]);
    }

    #[test]
    fn from_typing_import_multiple() {
        let imports = parser().parse_imports("from typing import List, Dict, Optional");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "typing");
        assert_eq!(imports[0].names, vec!["List", "Dict", "Optional"]);
    }

    #[test]
    fn relative_import() {
        let imports = parser().parse_imports("from . import utils");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, ".");
        assert_eq!(imports[0].names, vec!["utils"]);
    }

    #[test]
    fn relative_import_dotdot() {
        let imports = parser().parse_imports("from ..models import User");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "..models");
        assert_eq!(imports[0].names, vec!["User"]);
    }

    #[test]
    fn multiline_import() {
        let source = "from typing import (\n    List,\n    Dict,\n    Optional,\n)";
        let imports = parser().parse_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "typing");
        assert!(imports[0].names.contains(&"List".to_string()));
        assert!(imports[0].names.contains(&"Dict".to_string()));
        assert!(imports[0].names.contains(&"Optional".to_string()));
    }

    #[test]
    fn def_with_return_type() {
        let source = "def greet(name: str) -> str:\n    return f'Hello {name}'";
        let fns = parser().parse_functions(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "greet");
        assert_eq!(fns[0].params, vec!["name"]);
        assert_eq!(fns[0].return_type.as_deref(), Some("str"));
        assert_eq!(fns[0].line, 1);
    }

    #[test]
    fn async_def_function() {
        let source = "async def fetch() -> Response:\n    pass";
        let fns = parser().parse_functions(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "fetch");
        assert!(fns[0].params.is_empty());
        assert_eq!(fns[0].return_type.as_deref(), Some("Response"));
    }

    #[test]
    fn class_as_export() {
        let source = "class MyService:\n    pass";
        let exports = parser().parse_exports(source);
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].name, "MyService");
        assert_eq!(exports[0].kind, ExportKind::Named);
    }

    #[test]
    fn class_with_base_as_export() {
        let source = "class MyService(BaseService):\n    pass";
        let exports = parser().parse_exports(source);
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].name, "MyService");
    }

    #[test]
    fn top_level_def_as_export() {
        let source = "def main():\n    pass";
        let exports = parser().parse_exports(source);
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].name, "main");
    }

    #[test]
    fn indented_def_not_exported() {
        let source = "class Foo:\n    def method(self):\n        pass";
        let exports = parser().parse_exports(source);
        // Only Foo should be exported, not method
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].name, "Foo");
    }

    #[test]
    fn empty_source() {
        let p = parser();
        assert!(p.parse_imports("").is_empty());
        assert!(p.parse_exports("").is_empty());
        assert!(p.parse_functions("").is_empty());
    }

    #[test]
    fn self_param_excluded() {
        let source = "def method(self, x: int) -> None:\n    pass";
        let fns = parser().parse_functions(source);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].params, vec!["x"]);
    }

    #[test]
    fn import_with_alias() {
        let imports = parser().parse_imports("import numpy as np");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "numpy");
    }
}
