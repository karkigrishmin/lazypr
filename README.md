# lazypr

[![CI](https://github.com/karkigrishmin/lazypr/actions/workflows/ci.yml/badge.svg)](https://github.com/karkigrishmin/lazypr/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](https://www.rust-lang.org/)
[![GitHub release](https://img.shields.io/github/v/release/karkigrishmin/lazypr)](https://github.com/karkigrishmin/lazypr/releases)

**The code review tool for the AI era.** A terminal-native TUI that helps you actually review the code that AI writes — not just skim and approve.

![lazypr review TUI](docs/demo-review.gif)

## The Problem

AI coding tools generate hundreds of lines in minutes. A developer prompts "add authentication" and gets 15 new files. The code compiles, tests pass, CI is green.

**But nobody actually reads it.**

The PR gets a "LGTM" and ships. Then you discover the AI hallucinated an API, duplicated logic that already existed, or introduced a subtle security hole buried in file 47 of 50.

GitHub's review UI makes this worse:
- **50 files in alphabetical order** — you waste time on lockfiles before reaching actual logic
- **No sense of priority** — every file looks equally important
- **No structural awareness** — "200 lines added" doesn't tell you if that's 1 new function or 15
- **No memory** — you reviewed 30 files yesterday, but today they all show as unread
- **No move detection** — the AI refactored code into a new file, and you're re-reviewing identical code shown as "added"

## How lazypr Helps

lazypr is a terminal TUI that uses static analysis — not AI — to help you review code intelligently.

### Know What Matters First

Files are sorted by **review priority**, not alphabetically. Each file gets a heatmap score based on:
- How many logic lines changed (not just line count)
- File category (source > test > config > generated > lockfile)
- Git churn (frequently-changed files are riskier)
- Function-level changes (`+3fn ~1sig -1fn` — 3 functions added, 1 signature changed)

Skip the lockfiles. Start with the code that matters.

### See What Actually Changed

- **Three-color diff** — Green (added), Red (removed), **Cyan (moved)**. When the AI moves code between files, you see it immediately instead of re-reading identical code.
- **Semantic diff** — Not just "200 lines added" but "+8 functions added, 1 signature changed, 2 deleted". You know the structural impact at a glance.
- **Syntax highlighting** — Full language-aware highlighting in the diff view.

### Don't Lose Your Place

- **Session tracking** — Mark files as viewed. Come back tomorrow, pick up where you left off.
- **Inter-diff** — The author pushed fixes. Press `i` to see only what changed since your last review, not the entire PR again.
- **Private notes** — Press `n` to attach notes to specific lines. They persist locally across sessions.

### Catch Issues Before They Ship

```
$ lazypr ghost

Ghost analysis: feature/auth vs main

ERRORS (1):
  [BROKEN_IMPORT] src/components/UserBadge.tsx
    imports deleted file src/utils/auth-helpers.ts

WARNINGS (3):
  [MISSING_TEST] src/hooks/useAuth.ts — no test file
  [MISSING_TEST] src/utils/permissions.ts — no test file
  [HIGH_IMPACT] src/types/user.ts — 8 files depend on this
```

### Split Massive PRs

The AI generated 50 files in one PR? Split it into dependency-ordered groups:

```
$ lazypr split --dry-run

Split plan: 4 groups, 2 skipped files

  Group 1: core/types (3 files, 120 lines)
  Group 2: hooks (4 files, 180 lines) [depends on: 1]
  Group 3: components (5 files, 200 lines) [depends on: 1, 2]
  Group 4: pages (3 files, 90 lines) [depends on: 3]
```

Add `--execute` to create stacked branches. Add `--create-prs` to create GitHub draft PRs.

## Why Not Just `git diff`?

| | `git diff` | lazypr |
|---|---|---|
| File ordering | Alphabetical | By review priority |
| Moved code | Shows as delete + add | Cyan = moved, skip it |
| Function changes | Line-level only | `+3fn ~1sig` semantic summary |
| Session memory | None | Remembers viewed files |
| Broken imports | Not detected | `lazypr ghost` catches them |
| PR splitting | Manual | Auto-grouped by dependency |

## Installation

```bash
git clone https://github.com/karkigrishmin/lazypr.git
cd lazypr
cargo install --path .
```

Or build manually:

```bash
cargo build --release
# Binary at target/release/lazypr
```

## Quick Start

```bash
cd your-project
git checkout your-feature-branch

lazypr review              # Interactive TUI
lazypr review --json       # JSON output
lazypr ghost               # Pre-push analysis
lazypr impact src/file.ts  # Dependency impact
lazypr split --dry-run     # Split plan
lazypr inbox               # GitHub PR dashboard
```

## Example Output

![lazypr CLI commands](docs/demo-ghost.gif)

## Commands

| Command | Description |
|---------|-------------|
| `lazypr review` | Interactive TUI review (default) |
| `lazypr ghost` | Pre-push analysis — broken imports, missing tests, high-impact |
| `lazypr impact <file>` | Show dependency impact for a file |
| `lazypr split` | Generate a split plan for the PR |
| `lazypr split --execute --create-prs` | Create stacked branches + GitHub draft PRs |
| `lazypr inbox` | PR inbox — your PRs and review requests |

### Global Flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON instead of TUI |
| `--base <ref>` | Override base branch (default: auto-detect) |

## TUI Keybindings

| Key | Action |
|-----|--------|
| `j`/`k` | Navigate up/down |
| `Tab` | Switch panes |
| `s` | Skip file |
| `v` | Mark file as viewed |
| `/` | Search files |
| `n` | Add note (in diff pane) |
| `c` | Open checklist |
| `i` | Toggle inter-diff mode |
| `Ctrl-d`/`Ctrl-u` | Half-page scroll |
| `g`/`G` | Jump to top/bottom |
| `?` | Help |
| `q` | Quit |

## Configuration

### `.lazypr/config.yml`

```yaml
base_branch: main

review:
  move_detection_min_lines: 3
  move_similarity_threshold: 0.85
  skip_patterns:
    - "*.snap"
    - "package-lock.json"

split:
  target_group_size: 150
  max_group_size: 400

display:
  theme: auto
  syntax_highlighting: true
```

### `.lazypr/checklist.yml`

```yaml
- when: "src/hooks/*"
  checks:
    - "Cleanup in useEffect?"
    - "Error handling for async?"
    - "Tests added?"
```

## Architecture

```
┌─────────────────────────────────────────────┐
│           TUI Layer (ratatui)               │
│  Review  Split  Inbox  Ghost               │
├─────────────────────────────────────────────┤
│           Command Layer                     │
│  review  split  ghost  impact  inbox       │
├─────────────────────────────────────────────┤
│           Core Engine (no IO)               │
│  Differ  Graph  Splitter  Analyzer  Parser │
├──────────────────┬──────────────────────────┤
│  Git (git2)      │  State (.lazypr/)        │
├──────────────────┴──────────────────────────┤
│  Remote (optional) — GitHub via octocrab    │
└─────────────────────────────────────────────┘
```

## Design Philosophy

- **Local-first** — Everything computed from git. Works offline, on a plane, during GitHub outages.
- **Deterministic** — Same input, same output. No probabilistic responses.
- **Zero config** — Running `lazypr` in a git repo does something useful immediately.
- **Composable** — Every command supports `--json`. Pipe to jq, to CI, to other tools.
- **No AI** — Intelligence comes from static analysis, graph theory, and git history.

## What's Next

- [ ] Inline PR comments — read and post GitHub review comments directly in the TUI
- [ ] Review from PR URL — `lazypr review https://github.com/org/repo/pull/123`
- [ ] CI integration — `lazypr ghost --ci` posts findings as a PR comment
- [ ] Shared config — `.lazypr/config.yml` checked into repo for team-wide settings

## Multi-Language Support

| Language | Parser | Capabilities |
|----------|--------|-------------|
| TypeScript/JavaScript | tree-sitter | Imports, exports, functions, semantic diff |
| Python | Regex | Imports, functions, classes |
| Rust | Regex | use/mod, functions, pub types |
| Other | Generic regex | Basic import/function detection |

## Tech Stack

Rust 2021 · ratatui · git2 · octocrab · tree-sitter · petgraph · syntect · xxhash · tokio

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## License

MIT — see [LICENSE](LICENSE).
