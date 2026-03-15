use crate::core::types::{Export, FunctionSignature, Import, Language};

use super::LanguageParser;

/// Tree-sitter based parser for TypeScript and JavaScript files.
pub struct TypeScriptParser;

impl LanguageParser for TypeScriptParser {
    fn language(&self) -> Language {
        Language::TypeScript
    }

    fn can_parse(&self, extension: &str) -> bool {
        matches!(extension, "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs")
    }

    fn parse_imports(&self, _source: &str) -> Vec<Import> {
        Vec::new() // TODO: implement in Task 3
    }

    fn parse_exports(&self, _source: &str) -> Vec<Export> {
        Vec::new()
    }

    fn parse_functions(&self, _source: &str) -> Vec<FunctionSignature> {
        Vec::new()
    }
}
