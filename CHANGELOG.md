# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-03-15

### Added

#### Review TUI
- Interactive terminal UI with file tree (30%) and diff view (70%) split layout
- Heatmap scoring — files sorted by review priority (Deep/Scan/Glance/Skip)
- Three-color diff — green (added), red (removed), cyan (moved between files)
- Syntax highlighting via syntect for all supported languages
- Move detection using xxh3 content hashing and fuzzy similarity matching
- File classification — auto-detects Source, Test, Config, Lock, Generated, Snapshot, etc.
- Semantic diff — function-level change detection shown inline (`+2fn ~1sig -1fn`)
- File search with `/` key
- Skip (`s`) and mark viewed (`v`) per file
- Review progress bar (X/Y files viewed)
- Page scrolling (Ctrl-d/u, g/G)
- Help overlay (`?`)

#### Review State
- Session persistence — saves reviewed SHA, files viewed, timestamps
- Inter-diff mode (`i`) — show only changes since last review
- Private line-level notes (`n` key) — persisted in `.lazypr/`
- Project-specific checklists (`c` key) — loaded from `.lazypr/checklist.yml`
- File churn analysis — git history risk multiplier in heatmap scoring

#### Pre-Push Analysis
- `lazypr ghost` — detect broken imports, missing tests, high-impact changes
- `lazypr impact <file>` — show direct + transitive dependents
- Dependency graph built from parsed imports using petgraph

#### Smart Split
- `lazypr split` — auto-group files by dependency order and size budget
- Topological sort + SCC detection for circular dependencies
- Split plan validation for import consistency
- `--dry-run` to preview, `--execute` to create stacked branches
- Interactive split TUI screen (tab 2)

#### Parsers
- TypeScript/JavaScript — tree-sitter AST parser
- Python — regex-based parser
- Rust — regex-based parser
- Generic — fallback regex parser for any language

#### Infrastructure
- JSON output (`--json`) for all commands
- Configurable via `.lazypr/config.yml`
- 289 tests with full CI pipeline

[0.1.0]: https://github.com/karkigrishmin/lazypr/releases/tag/v0.1.0
