use std::path::Path;

use crate::core::FileCategory;

/// Classify a file path into a [`FileCategory`] using pattern-matching.
///
/// The first matching rule wins:
/// 1. Lock
/// 2. Generated
/// 3. Snapshot
/// 4. Test
/// 5. TypeDefinition
/// 6. Config
/// 7. Style
/// 8. Documentation
/// 9. Source (fallback)
pub fn classify_file(path: &str) -> FileCategory {
    let p = Path::new(path);
    let filename = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");

    // 1. Lock — exact filename match
    const LOCK_FILES: &[&str] = &[
        "package-lock.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "Cargo.lock",
        "Gemfile.lock",
        "poetry.lock",
        "composer.lock",
        "go.sum",
    ];
    if LOCK_FILES.contains(&filename) {
        return FileCategory::Lock;
    }

    // 2. Generated
    // *.generated.* — filename contains ".generated."
    // *.d.ts        — filename ends with ".d.ts"
    // path contains /codegen/ or /generated/
    // *.pb.go       — filename ends with ".pb.go"
    // *_generated.* — filename contains "_generated."
    if filename.contains(".generated.")
        || filename.ends_with(".d.ts")
        || path.contains("/codegen/")
        || path.contains("/generated/")
        || filename.ends_with(".pb.go")
        || filename.contains("_generated.")
    {
        return FileCategory::Generated;
    }

    // 3. Snapshot
    // *.snap, *.snapshot, path contains /__snapshots__/
    if ext == "snap" || ext == "snapshot" || path.contains("/__snapshots__/") {
        return FileCategory::Snapshot;
    }

    // 4. Test
    // *.test.*, *.spec.*, *_test.go, *_test.rs
    // path contains /__tests__/ or /test/ or /tests/
    if filename.contains(".test.")
        || filename.contains(".spec.")
        || filename.ends_with("_test.go")
        || filename.ends_with("_test.rs")
        || path.contains("/__tests__/")
        || path.contains("/test/")
        || path.contains("/tests/")
        || path.starts_with("test/")
        || path.starts_with("tests/")
    {
        return FileCategory::Test;
    }

    // 5. TypeDefinition
    // path contains /types/ or /interfaces/
    // *.types.ts, *.types.js
    if path.contains("/types/")
        || path.contains("/interfaces/")
        || filename.ends_with(".types.ts")
        || filename.ends_with(".types.js")
    {
        return FileCategory::TypeDefinition;
    }

    // 6. Config
    // *.config.*
    // filename starts with .eslintrc or .prettierrc
    // tsconfig*.json — filename starts with "tsconfig" and ends with ".json"
    // .env*          — filename starts with ".env"
    // exact Makefile / Dockerfile
    // docker-compose*
    if filename.contains(".config.")
        || filename.starts_with(".eslintrc")
        || filename.starts_with(".prettierrc")
        || (filename.starts_with("tsconfig") && filename.ends_with(".json"))
        || filename.starts_with(".env")
        || filename == "Makefile"
        || filename == "Dockerfile"
        || filename.starts_with("docker-compose")
    {
        return FileCategory::Config;
    }

    // 7. Style
    // extension css/scss/less
    // *.module.css — filename ends with ".module.css"
    // *.styled.*   — filename contains ".styled."
    if matches!(ext, "css" | "scss" | "less")
        || filename.ends_with(".module.css")
        || filename.contains(".styled.")
    {
        return FileCategory::Style;
    }

    // 8. Documentation
    // extension md/mdx/txt
    // filename starts with LICENSE or CHANGELOG
    if matches!(ext, "md" | "mdx" | "txt")
        || filename.starts_with("LICENSE")
        || filename.starts_with("CHANGELOG")
    {
        return FileCategory::Documentation;
    }

    // 9. Source (fallback)
    FileCategory::Source
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lockfiles_are_classified_as_lock() {
        assert_eq!(classify_file("package-lock.json"), FileCategory::Lock);
        assert_eq!(classify_file("yarn.lock"), FileCategory::Lock);
        assert_eq!(classify_file("Cargo.lock"), FileCategory::Lock);
        assert_eq!(classify_file("poetry.lock"), FileCategory::Lock);
        assert_eq!(classify_file("go.sum"), FileCategory::Lock);
        assert_eq!(classify_file("pnpm-lock.yaml"), FileCategory::Lock);
        assert_eq!(classify_file("Gemfile.lock"), FileCategory::Lock);
        assert_eq!(classify_file("composer.lock"), FileCategory::Lock);
    }

    #[test]
    fn generated_files_are_detected() {
        assert_eq!(
            classify_file("src/api.generated.ts"),
            FileCategory::Generated
        );
        assert_eq!(classify_file("types/index.d.ts"), FileCategory::Generated);
        assert_eq!(
            classify_file("src/codegen/output.rs"),
            FileCategory::Generated
        );
        assert_eq!(
            classify_file("proto/message.pb.go"),
            FileCategory::Generated
        );
        assert_eq!(
            classify_file("schema_generated.rs"),
            FileCategory::Generated
        );
    }

    #[test]
    fn snapshot_files_are_detected() {
        assert_eq!(classify_file("tests/login.snap"), FileCategory::Snapshot);
        assert_eq!(
            classify_file("src/__snapshots__/App.test.tsx.snap"),
            FileCategory::Snapshot
        );
        assert_eq!(
            classify_file("tests/output.snapshot"),
            FileCategory::Snapshot
        );
    }

    #[test]
    fn test_files_are_detected() {
        assert_eq!(classify_file("src/utils.test.ts"), FileCategory::Test);
        assert_eq!(classify_file("src/App.spec.tsx"), FileCategory::Test);
        assert_eq!(classify_file("src/__tests__/helper.ts"), FileCategory::Test);
        assert_eq!(
            classify_file("tests/integration/login.rs"),
            FileCategory::Test
        );
        assert_eq!(classify_file("pkg/handler_test.go"), FileCategory::Test);
        assert_eq!(classify_file("src/parser_test.rs"), FileCategory::Test);
    }

    #[test]
    fn type_definitions_are_detected() {
        assert_eq!(
            classify_file("src/types/user.ts"),
            FileCategory::TypeDefinition
        );
        assert_eq!(
            classify_file("src/models.types.ts"),
            FileCategory::TypeDefinition
        );
        assert_eq!(
            classify_file("lib/interfaces/api.ts"),
            FileCategory::TypeDefinition
        );
    }

    #[test]
    fn config_files_are_detected() {
        assert_eq!(classify_file("jest.config.ts"), FileCategory::Config);
        assert_eq!(classify_file(".eslintrc.json"), FileCategory::Config);
        assert_eq!(classify_file(".prettierrc"), FileCategory::Config);
        assert_eq!(classify_file("tsconfig.base.json"), FileCategory::Config);
        assert_eq!(classify_file(".env.production"), FileCategory::Config);
        assert_eq!(classify_file("Makefile"), FileCategory::Config);
        assert_eq!(classify_file("Dockerfile"), FileCategory::Config);
        assert_eq!(classify_file("docker-compose.yml"), FileCategory::Config);
    }

    #[test]
    fn style_files_are_detected() {
        assert_eq!(classify_file("src/App.css"), FileCategory::Style);
        assert_eq!(classify_file("styles/main.scss"), FileCategory::Style);
        assert_eq!(classify_file("theme.less"), FileCategory::Style);
        assert_eq!(classify_file("src/Button.module.css"), FileCategory::Style);
        assert_eq!(classify_file("src/Box.styled.ts"), FileCategory::Style);
    }

    #[test]
    fn documentation_files_are_detected() {
        assert_eq!(classify_file("README.md"), FileCategory::Documentation);
        assert_eq!(classify_file("docs/guide.mdx"), FileCategory::Documentation);
        assert_eq!(classify_file("notes.txt"), FileCategory::Documentation);
        assert_eq!(classify_file("LICENSE"), FileCategory::Documentation);
        assert_eq!(classify_file("CHANGELOG.md"), FileCategory::Documentation);
    }

    #[test]
    fn source_files_are_the_default() {
        assert_eq!(classify_file("src/main.rs"), FileCategory::Source);
        assert_eq!(
            classify_file("src/components/App.tsx"),
            FileCategory::Source
        );
        assert_eq!(classify_file("lib/utils.py"), FileCategory::Source);
        assert_eq!(classify_file("cmd/server/main.go"), FileCategory::Source);
    }

    #[test]
    fn nested_paths_are_classified_correctly() {
        assert_eq!(
            classify_file("vendor/package-lock.json"),
            FileCategory::Lock
        );
        assert_eq!(
            classify_file("packages/core/tests/unit/parser.rs"),
            FileCategory::Test
        );
    }
}
