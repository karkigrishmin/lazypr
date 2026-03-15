use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::store::store_path;

/// Top-level configuration for lazypr.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LazyprConfig {
    /// The base branch to diff against (e.g. `"main"` or `"develop"`).
    #[serde(default = "default_base_branch")]
    pub base_branch: String,

    /// Settings that control the review analysis engine.
    #[serde(default)]
    pub review: ReviewConfig,

    /// Settings that control how diffs are split into review groups.
    #[serde(default)]
    pub split: SplitConfig,

    /// Settings that control terminal display.
    #[serde(default)]
    pub display: DisplayConfig,
}

/// Configuration for the review / analysis engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewConfig {
    /// Minimum number of contiguous lines before move detection kicks in.
    #[serde(default = "default_move_min_lines")]
    pub move_detection_min_lines: usize,

    /// Similarity threshold (0.0 -- 1.0) for treating a block as "moved".
    #[serde(default = "default_move_threshold")]
    pub move_similarity_threshold: f64,

    /// Glob patterns for files that should be skipped during review.
    #[serde(default)]
    pub skip_patterns: Vec<String>,
}

/// Configuration for the diff splitter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitConfig {
    /// Target number of logic lines per review group.
    #[serde(default = "default_target_group_size")]
    pub target_group_size: usize,

    /// Hard maximum number of logic lines per review group.
    #[serde(default = "default_max_group_size")]
    pub max_group_size: usize,
}

/// Configuration for terminal display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    /// Color theme name (`"auto"`, `"dark"`, `"light"`).
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Whether to enable syntax highlighting in the TUI.
    #[serde(default = "default_true")]
    pub syntax_highlighting: bool,

    /// Whether to default to side-by-side diff view.
    #[serde(default)]
    pub side_by_side: bool,
}

// ---- default value helpers ----

fn default_base_branch() -> String {
    "main".to_owned()
}

fn default_move_min_lines() -> usize {
    3
}

fn default_move_threshold() -> f64 {
    0.85
}

fn default_target_group_size() -> usize {
    150
}

fn default_max_group_size() -> usize {
    400
}

fn default_theme() -> String {
    "auto".to_owned()
}

fn default_true() -> bool {
    true
}

// ---- Default impls ----

impl Default for LazyprConfig {
    fn default() -> Self {
        Self {
            base_branch: default_base_branch(),
            review: ReviewConfig::default(),
            split: SplitConfig::default(),
            display: DisplayConfig::default(),
        }
    }
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            move_detection_min_lines: default_move_min_lines(),
            move_similarity_threshold: default_move_threshold(),
            skip_patterns: Vec::new(),
        }
    }
}

impl Default for SplitConfig {
    fn default() -> Self {
        Self {
            target_group_size: default_target_group_size(),
            max_group_size: default_max_group_size(),
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            syntax_highlighting: default_true(),
            side_by_side: false,
        }
    }
}

// ---- load / save ----

impl LazyprConfig {
    /// Load configuration from `.lazypr/config.yml`.
    ///
    /// Falls back to `Default` when the file does not exist.
    pub fn load(repo_root: &Path) -> Result<Self> {
        let path = store_path(repo_root).join("config.yml");
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let config: Self = serde_yaml::from_str(&contents)
            .with_context(|| format!("parsing {}", path.display()))?;
        Ok(config)
    }

    /// Write configuration to `.lazypr/config.yml`.
    #[allow(dead_code)]
    pub fn save(&self, repo_root: &Path) -> Result<()> {
        let base = store_path(repo_root);
        std::fs::create_dir_all(&base).with_context(|| format!("creating {}", base.display()))?;
        let path = base.join("config.yml");
        let yaml = serde_yaml::to_string(self).context("serialising config to YAML")?;
        std::fs::write(&path, yaml).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn default_config_has_correct_values() {
        let cfg = LazyprConfig::default();
        assert_eq!(cfg.base_branch, "main");
        assert_eq!(cfg.review.move_detection_min_lines, 3);
        assert!((cfg.review.move_similarity_threshold - 0.85).abs() < f64::EPSILON);
        assert!(cfg.review.skip_patterns.is_empty());
        assert_eq!(cfg.split.target_group_size, 150);
        assert_eq!(cfg.split.max_group_size, 400);
        assert_eq!(cfg.display.theme, "auto");
        assert!(cfg.display.syntax_highlighting);
        assert!(!cfg.display.side_by_side);
    }

    #[test]
    fn load_falls_back_to_default_when_no_file() {
        let tmp = TempDir::new().expect("create temp dir");
        let cfg = LazyprConfig::load(tmp.path()).expect("load");
        assert_eq!(cfg.base_branch, "main");
    }

    #[test]
    fn load_reads_yaml_file() {
        let tmp = TempDir::new().expect("create temp dir");
        let dir = tmp.path().join(".lazypr");
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(
            dir.join("config.yml"),
            "base_branch: develop\nsplit:\n  target_group_size: 200\n",
        )
        .expect("write");

        let cfg = LazyprConfig::load(tmp.path()).expect("load");
        assert_eq!(cfg.base_branch, "develop");
        assert_eq!(cfg.split.target_group_size, 200);
    }

    #[test]
    fn partial_yaml_uses_defaults_for_missing_fields() {
        let tmp = TempDir::new().expect("create temp dir");
        let dir = tmp.path().join(".lazypr");
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join("config.yml"), "base_branch: staging\n").expect("write");

        let cfg = LazyprConfig::load(tmp.path()).expect("load");
        assert_eq!(cfg.base_branch, "staging");
        // Everything else should be default
        assert_eq!(cfg.review.move_detection_min_lines, 3);
        assert_eq!(cfg.split.max_group_size, 400);
        assert!(cfg.display.syntax_highlighting);
    }

    #[test]
    fn save_then_load_round_trip() {
        let tmp = TempDir::new().expect("create temp dir");
        let mut cfg = LazyprConfig::default();
        cfg.base_branch = "develop".to_owned();
        cfg.split.target_group_size = 300;
        cfg.display.side_by_side = true;
        cfg.review.skip_patterns = vec!["*.lock".to_owned()];

        cfg.save(tmp.path()).expect("save");
        let loaded = LazyprConfig::load(tmp.path()).expect("load");

        assert_eq!(loaded.base_branch, "develop");
        assert_eq!(loaded.split.target_group_size, 300);
        assert!(loaded.display.side_by_side);
        assert_eq!(loaded.review.skip_patterns, vec!["*.lock".to_owned()]);
    }
}
