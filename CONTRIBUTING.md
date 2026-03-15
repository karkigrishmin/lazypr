# Contributing to lazypr

Thanks for your interest in contributing! Here's how to get started.

## Development Setup

```bash
# Clone the repo
git clone https://github.com/karkigrishmin/lazypr.git
cd lazypr

# Build
cargo build

# Run tests
cargo test

# Run the tool
cargo run -- review
```

## Before Submitting a PR

Run all checks:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

All three must pass with zero warnings.

## Commit Style

We use [Conventional Commits](https://www.conventionalcommits.org/):

- `feat: add new feature`
- `fix: fix a bug`
- `refactor: restructure code without behavior change`
- `test: add or update tests`
- `docs: update documentation`
- `chore: maintenance tasks`

Keep commit messages as a single line.

## Architecture

```
src/
├── cli.rs           # Clap CLI definitions
├── main.rs          # Entry point
├── commands/        # Command handlers (thin wrappers)
├── core/            # Core engine (NO IO)
│   ├── analyzer/    # File classification, ghost analysis
│   ├── differ/      # Diff pipeline, heatmap, semantic diff
│   ├── git/         # git2 operations
│   ├── graph/       # Dependency graph, impact analysis
│   ├── parser/      # Language parsers (tree-sitter + regex)
│   ├── splitter/    # Split algorithm, executor, validator
│   └── types.rs     # All data types
├── state/           # Persistence (.lazypr/ directory)
└── tui/             # Terminal UI (ratatui)
    ├── screens/     # Review, Split, Ghost screens
    └── widgets/     # File tree, diff view, etc.
```

**Key rule:** Core engine modules must NOT do IO. They receive data as parameters and return structured results.

## Adding a New Parser

1. Create `src/core/parser/your_language.rs`
2. Implement the `LanguageParser` trait
3. Add to `parser_for()` in `src/core/parser/mod.rs`
4. Add tests

## Questions?

Open an issue — we're happy to help.
