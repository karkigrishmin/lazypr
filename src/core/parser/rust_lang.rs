use crate::core::types::{Export, FunctionSignature, Import, Language};

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
