use crate::core::types::{Export, FunctionSignature, Import, Language};

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

    fn parse_imports(&self, _source: &str) -> Vec<Import> {
        Vec::new() // TODO: implement in Task 4
    }

    fn parse_exports(&self, _source: &str) -> Vec<Export> {
        Vec::new()
    }

    fn parse_functions(&self, _source: &str) -> Vec<FunctionSignature> {
        Vec::new()
    }
}
