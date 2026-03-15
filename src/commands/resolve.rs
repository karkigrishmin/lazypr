#![allow(dead_code)]

use std::path::Path;

use anyhow::Result;
use ignore::WalkBuilder;

use crate::core::parser::parser_for;
use crate::core::types::{Language, ParsedFile};

/// Supported source-file extensions for repo scanning.
const SUPPORTED_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx", "mjs", "cjs", "py", "rs"];

/// Resolve a relative import source to a file path relative to the repo root.
/// Returns `None` for external packages (non-relative imports) or if the
/// resolved path does not exist on disk.
pub fn resolve_import(
    source: &str,
    from_file: &str,
    repo_root: &Path,
    language: &Language,
) -> Option<String> {
    // External package — not a relative import.
    if !source.starts_with('.') {
        return None;
    }

    // Directory containing the importing file.
    let from_dir = Path::new(from_file)
        .parent()
        .unwrap_or_else(|| Path::new(""));

    // Build the candidate path by joining the from-dir with the import source.
    let raw = from_dir.join(source);

    // Normalise away `.` / `..` segments without touching the filesystem.
    let base = normalize_path(&raw);

    // Candidate extensions to try, in priority order.
    let extensions: &[&str] = match language {
        Language::TypeScript | Language::JavaScript => &[
            ".ts",
            ".tsx",
            ".js",
            ".jsx", //
            "/index.ts",
            "/index.tsx",
            "/index.js",
            "/index.jsx",
        ],
        Language::Python => &[".py", "/__init__.py"],
        Language::Rust => &[".rs", "/mod.rs"],
        Language::Unknown => return None,
    };

    for ext in extensions {
        let candidate = format!("{}{}", base, ext);
        if repo_root.join(&candidate).is_file() {
            return Some(candidate);
        }
    }

    // Maybe the import already has an extension that exists.
    if repo_root.join(&base).is_file() {
        return Some(base);
    }

    None
}

/// Read and parse all supported source files in the repository.
/// Respects `.gitignore` via the `ignore` crate.  After parsing, import
/// paths are resolved where possible.
pub fn parse_repo_files(repo_root: &Path) -> Result<Vec<ParsedFile>> {
    let walker = WalkBuilder::new(repo_root).build();

    let mut parsed_files: Vec<ParsedFile> = Vec::new();

    for entry in walker {
        let entry = entry?;
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        if !SUPPORTED_EXTENSIONS.contains(&ext) {
            continue;
        }

        // Path relative to repo root.
        let rel_path = path
            .strip_prefix(repo_root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let content = std::fs::read_to_string(path)?;
        let parser = parser_for(&rel_path);
        let parsed = parser.parse(&rel_path, &content);
        parsed_files.push(parsed);
    }

    // Second pass: resolve imports.
    resolve_all_imports(&mut parsed_files, repo_root);

    Ok(parsed_files)
}

/// Walk every `ParsedFile` and attempt to fill in `resolved_path` for each
/// import.
fn resolve_all_imports(files: &mut [ParsedFile], repo_root: &Path) {
    for file in files.iter_mut() {
        let language = file.language.clone();
        let file_path = file.path.clone();
        for import in file.imports.iter_mut() {
            import.resolved_path = resolve_import(&import.source, &file_path, repo_root, &language);
        }
    }
}

/// Normalise a path by resolving `.` and `..` components purely lexically
/// (no filesystem access).  Returns a String using forward-slash separators.
fn normalize_path(path: &Path) -> String {
    let mut components: Vec<&str> = Vec::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => { /* skip `.` */ }
            std::path::Component::ParentDir => {
                components.pop();
            }
            _ => {
                components.push(comp.as_os_str().to_str().unwrap_or(""));
            }
        }
    }
    components.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn resolve_relative_ts_import() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/utils.ts"), "export const x = 1;").unwrap();

        let result = resolve_import("./utils", "src/app.ts", tmp.path(), &Language::TypeScript);
        assert_eq!(result, Some("src/utils.ts".to_string()));
    }

    #[test]
    fn resolve_index_file() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src/components")).unwrap();
        std::fs::write(tmp.path().join("src/components/index.ts"), "export {};").unwrap();

        let result = resolve_import(
            "./components",
            "src/app.ts",
            tmp.path(),
            &Language::TypeScript,
        );
        assert_eq!(result, Some("src/components/index.ts".to_string()));
    }

    #[test]
    fn external_package_returns_none() {
        let tmp = TempDir::new().unwrap();
        let result = resolve_import("react", "src/app.ts", tmp.path(), &Language::TypeScript);
        assert!(result.is_none());
    }

    #[test]
    fn parent_dir_import() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src/utils")).unwrap();
        std::fs::write(tmp.path().join("src/helpers.ts"), "export {};").unwrap();

        let result = resolve_import(
            "../helpers",
            "src/utils/index.ts",
            tmp.path(),
            &Language::TypeScript,
        );
        assert_eq!(result, Some("src/helpers.ts".to_string()));
    }

    #[test]
    fn python_import_resolution() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("app")).unwrap();
        std::fs::write(tmp.path().join("app/models.py"), "class User: pass").unwrap();

        let result = resolve_import("./models", "app/views.py", tmp.path(), &Language::Python);
        assert_eq!(result, Some("app/models.py".to_string()));
    }

    #[test]
    fn nonexistent_relative_returns_none() {
        let tmp = TempDir::new().unwrap();
        let result = resolve_import(
            "./nonexistent",
            "src/app.ts",
            tmp.path(),
            &Language::TypeScript,
        );
        assert!(result.is_none());
    }

    #[test]
    fn rust_import_resolution() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/utils.rs"), "pub fn hello() {}").unwrap();

        let result = resolve_import("./utils", "src/main.rs", tmp.path(), &Language::Rust);
        assert_eq!(result, Some("src/utils.rs".to_string()));
    }

    #[test]
    fn rust_mod_rs_resolution() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src/core")).unwrap();
        std::fs::write(tmp.path().join("src/core/mod.rs"), "pub mod types;").unwrap();

        let result = resolve_import("./core", "src/lib.rs", tmp.path(), &Language::Rust);
        assert_eq!(result, Some("src/core/mod.rs".to_string()));
    }

    #[test]
    fn python_init_resolution() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("pkg/sub")).unwrap();
        std::fs::write(tmp.path().join("pkg/sub/__init__.py"), "").unwrap();

        let result = resolve_import("./sub", "pkg/main.py", tmp.path(), &Language::Python);
        assert_eq!(result, Some("pkg/sub/__init__.py".to_string()));
    }

    #[test]
    fn parse_repo_files_finds_supported_files() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/app.ts"),
            "import { x } from './utils';\nexport const y = x;",
        )
        .unwrap();
        std::fs::write(tmp.path().join("src/utils.ts"), "export const x = 1;").unwrap();
        // Non-supported file should be skipped.
        std::fs::write(tmp.path().join("src/readme.md"), "# Hello").unwrap();

        let files = parse_repo_files(tmp.path()).unwrap();
        assert_eq!(files.len(), 2);

        // All returned files should have supported languages.
        for f in &files {
            assert_ne!(f.language, Language::Unknown);
        }
    }
}
