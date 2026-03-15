use crate::core::types::{Export, FunctionSignature, Import, Language};

use super::LanguageParser;

/// Regex-based fallback parser for languages without a dedicated parser.
pub struct GenericParser;

impl LanguageParser for GenericParser {
    fn language(&self) -> Language {
        Language::Unknown
    }

    fn can_parse(&self, _extension: &str) -> bool {
        true // fallback parser accepts anything
    }

    fn parse_imports(&self, _source: &str) -> Vec<Import> {
        Vec::new() // TODO: implement in Task 2
    }

    fn parse_exports(&self, _source: &str) -> Vec<Export> {
        Vec::new()
    }

    fn parse_functions(&self, _source: &str) -> Vec<FunctionSignature> {
        Vec::new()
    }
}
