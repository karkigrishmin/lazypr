use std::path::Path;

use crate::core::types::{Export, FunctionSignature, Import, Language, ParsedFile};

pub mod generic;
pub mod python;
pub mod rust_lang;
pub mod typescript;

/// Trait that all language-specific parsers implement.
/// Receives source code as `&str` — no filesystem access.
pub trait LanguageParser: Send + Sync {
    /// Which language this parser handles.
    fn language(&self) -> Language;

    /// Whether this parser can handle files with the given extension.
    #[allow(dead_code)]
    fn can_parse(&self, extension: &str) -> bool;

    /// Extract import statements from source code.
    fn parse_imports(&self, source: &str) -> Vec<Import>;

    /// Extract export statements from source code.
    fn parse_exports(&self, source: &str) -> Vec<Export>;

    /// Extract function signatures from source code.
    fn parse_functions(&self, source: &str) -> Vec<FunctionSignature>;

    /// Full parse: convenience method that calls all three extractors.
    fn parse(&self, path: &str, source: &str) -> ParsedFile {
        ParsedFile {
            path: path.to_string(),
            language: self.language(),
            imports: self.parse_imports(source),
            exports: self.parse_exports(source),
            functions: self.parse_functions(source),
        }
    }
}

/// Detect programming language from a file path's extension.
pub fn detect_language(path: &str) -> Language {
    match Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
    {
        "ts" | "tsx" => Language::TypeScript,
        "js" | "jsx" | "mjs" | "cjs" => Language::JavaScript,
        "py" => Language::Python,
        "rs" => Language::Rust,
        _ => Language::Unknown,
    }
}

/// Return the appropriate parser for a given file path.
/// Falls back to `GenericParser` for unknown languages.
pub fn parser_for(path: &str) -> Box<dyn LanguageParser> {
    match detect_language(path) {
        Language::TypeScript | Language::JavaScript => Box::new(typescript::TypeScriptParser),
        Language::Python => Box::new(python::PythonParser),
        Language::Rust => Box::new(rust_lang::RustParser),
        Language::Unknown => Box::new(generic::GenericParser),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_typescript_extensions() {
        assert_eq!(detect_language("src/app.ts"), Language::TypeScript);
        assert_eq!(detect_language("src/App.tsx"), Language::TypeScript);
    }

    #[test]
    fn detect_javascript_extensions() {
        assert_eq!(detect_language("src/index.js"), Language::JavaScript);
        assert_eq!(detect_language("src/App.jsx"), Language::JavaScript);
        assert_eq!(detect_language("src/utils.mjs"), Language::JavaScript);
        assert_eq!(detect_language("src/config.cjs"), Language::JavaScript);
    }

    #[test]
    fn detect_python() {
        assert_eq!(detect_language("main.py"), Language::Python);
    }

    #[test]
    fn detect_rust() {
        assert_eq!(detect_language("src/lib.rs"), Language::Rust);
    }

    #[test]
    fn detect_unknown_returns_unknown() {
        assert_eq!(detect_language("Makefile"), Language::Unknown);
        assert_eq!(detect_language("README.md"), Language::Unknown);
        assert_eq!(detect_language("file.go"), Language::Unknown);
    }

    #[test]
    fn parser_for_ts_can_parse_ts() {
        let p = parser_for("app.ts");
        assert!(p.can_parse("ts"));
    }

    #[test]
    fn parser_for_js_can_parse_js() {
        let p = parser_for("app.js");
        assert!(p.can_parse("js"));
    }

    #[test]
    fn parser_for_py_can_parse_py() {
        let p = parser_for("app.py");
        assert!(p.can_parse("py"));
    }

    #[test]
    fn parser_for_rs_can_parse_rs() {
        let p = parser_for("app.rs");
        assert!(p.can_parse("rs"));
    }

    #[test]
    fn parser_for_unknown_returns_generic() {
        let p = parser_for("app.go");
        assert_eq!(p.language(), Language::Unknown);
    }
}
