//! YAML configuration types and stacked loading via VFS.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Builtin config embedded at compile time.
pub const BUILTIN_CONFIG_YAML: &str = include_str!("../builtin/config.yaml");

/// Full statusline configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct StatuslineConfig {
    /// Format string like "$directory $git_branch$git_status $model $context_bar"
    pub format: String,

    pub directory: DirectoryModuleConfig,
    pub git_branch: GitBranchModuleConfig,
    pub git_status: GitStatusModuleConfig,
    pub git_state: GitStateModuleConfig,
    pub model: ModelModuleConfig,
    pub context_bar: ContextBarModuleConfig,
    pub cost: CostModuleConfig,
    pub session: SessionModuleConfig,
    pub vim_mode: VimModeModuleConfig,
    pub agent: AgentModuleConfig,
    pub worktree: WorktreeModuleConfig,
    pub version: VersionModuleConfig,
    pub kanban: KanbanModuleConfig,
    pub index: IndexModuleConfig,
    pub languages: LanguagesModuleConfig,
}

impl Default for StatuslineConfig {
    /// Provides hardcoded defaults for each field.
    ///
    /// This must NOT call `serde_yaml_ng::from_str` because `StatuslineConfig` has
    /// `#[serde(default)]` on the struct, which means serde calls `Self::default()`
    /// during deserialization for any missing fields. If `default()` itself called
    /// `serde_yaml_ng::from_str`, that would trigger infinite recursion and a stack
    /// overflow.
    fn default() -> Self {
        Self {
            format: "$directory $git_branch$git_status$git_state $model $context_bar $kanban $index $languages".into(),
            directory: DirectoryModuleConfig::default(),
            git_branch: GitBranchModuleConfig::default(),
            git_status: GitStatusModuleConfig::default(),
            git_state: GitStateModuleConfig::default(),
            model: ModelModuleConfig::default(),
            context_bar: ContextBarModuleConfig::default(),
            cost: CostModuleConfig::default(),
            session: SessionModuleConfig::default(),
            vim_mode: VimModeModuleConfig::default(),
            agent: AgentModuleConfig::default(),
            worktree: WorktreeModuleConfig::default(),
            version: VersionModuleConfig::default(),
            kanban: KanbanModuleConfig::default(),
            index: IndexModuleConfig::default(),
            languages: LanguagesModuleConfig::default(),
        }
    }
}

/// Directory module configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct DirectoryModuleConfig {
    pub style: String,
    pub truncation_length: usize,
    pub format: String,
}

impl Default for DirectoryModuleConfig {
    fn default() -> Self {
        Self {
            style: "cyan bold".into(),
            truncation_length: 1,
            format: "$path".into(),
        }
    }
}

/// Git branch module configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct GitBranchModuleConfig {
    pub style: String,
    pub symbol: String,
    pub format: String,
    pub truncation_length: usize,
    pub truncation_symbol: String,
}

impl Default for GitBranchModuleConfig {
    fn default() -> Self {
        Self {
            style: "purple".into(),
            symbol: "\u{e0a0} ".into(),
            format: "$symbol$branch".into(),
            truncation_length: 20,
            truncation_symbol: "\u{2026}".into(),
        }
    }
}

/// Git status module configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct GitStatusModuleConfig {
    pub style: String,
    pub format: String,
    /// When true, append file counts after each symbol (e.g. "!2" instead of "!").
    pub show_counts: bool,
    pub modified: String,
    pub staged: String,
    pub untracked: String,
    pub deleted: String,
    pub conflicted: String,
    pub stashed: String,
    pub ahead: String,
    pub behind: String,
    pub diverged: String,
}

impl Default for GitStatusModuleConfig {
    fn default() -> Self {
        Self {
            style: "red bold".into(),
            format: "[$all_status$ahead_behind]".into(),
            show_counts: false,
            modified: "!".into(),
            staged: "+".into(),
            untracked: "?".into(),
            deleted: "\u{2718}".into(),
            conflicted: "=".into(),
            stashed: "$".into(),
            ahead: "\u{21e1}".into(),
            behind: "\u{21e3}".into(),
            diverged: "\u{21d5}".into(),
        }
    }
}

/// Git state module configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct GitStateModuleConfig {
    pub style: String,
    pub format: String,
}

impl Default for GitStateModuleConfig {
    fn default() -> Self {
        Self {
            style: "yellow bold".into(),
            format: "($state $progress)".into(),
        }
    }
}

/// Model module configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ModelModuleConfig {
    pub style: String,
    pub format: String,
}

impl Default for ModelModuleConfig {
    fn default() -> Self {
        Self {
            style: "green".into(),
            format: "\u{1f9e0} $name".into(),
        }
    }
}

/// Context bar module configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ContextBarModuleConfig {
    pub bar_width: usize,
    pub format: String,
    pub thresholds: ThresholdConfig,
}

impl Default for ContextBarModuleConfig {
    fn default() -> Self {
        Self {
            bar_width: 10,
            format: "[$bar] $percentage%".into(),
            thresholds: ThresholdConfig::default(),
        }
    }
}

/// Threshold configuration for context bar colors.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ThresholdConfig {
    pub low: ThresholdEntry,
    pub medium: ThresholdEntry,
    pub high: ThresholdEntry,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            low: ThresholdEntry {
                below: 50,
                style: "green".into(),
            },
            medium: ThresholdEntry {
                below: 80,
                style: "yellow".into(),
            },
            high: ThresholdEntry {
                below: 101,
                style: "red".into(),
            },
        }
    }
}

/// A single threshold entry with a cutoff value and style.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ThresholdEntry {
    pub below: u32,
    pub style: String,
}

/// Cost module configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct CostModuleConfig {
    pub style: String,
    pub format: String,
    pub hide_zero: bool,
}

impl Default for CostModuleConfig {
    fn default() -> Self {
        Self {
            style: "dim".into(),
            format: "$$amount".into(),
            hide_zero: true,
        }
    }
}

/// Session module configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SessionModuleConfig {
    pub style: String,
    pub format: String,
}

impl Default for SessionModuleConfig {
    fn default() -> Self {
        Self {
            style: "dim".into(),
            format: "$id".into(),
        }
    }
}

/// Vim mode module configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct VimModeModuleConfig {
    pub style: String,
    pub format: String,
}

impl Default for VimModeModuleConfig {
    fn default() -> Self {
        Self {
            style: "bold".into(),
            format: "$mode".into(),
        }
    }
}

/// Agent module configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AgentModuleConfig {
    pub style: String,
    pub format: String,
}

impl Default for AgentModuleConfig {
    fn default() -> Self {
        Self {
            style: "blue".into(),
            format: "\u{1f916} $name".into(),
        }
    }
}

/// Worktree module configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct WorktreeModuleConfig {
    pub style: String,
    pub format: String,
}

impl Default for WorktreeModuleConfig {
    fn default() -> Self {
        Self {
            style: "green".into(),
            format: "\u{1f332} $branch".into(),
        }
    }
}

/// Version module configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct VersionModuleConfig {
    pub style: String,
    pub format: String,
}

impl Default for VersionModuleConfig {
    fn default() -> Self {
        Self {
            style: "dim".into(),
            format: "v$version".into(),
        }
    }
}

/// Kanban module configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct KanbanModuleConfig {
    pub style: String,
    pub bar_width: usize,
    pub format: String,
    pub thresholds: KanbanThresholdConfig,
}

impl Default for KanbanModuleConfig {
    fn default() -> Self {
        Self {
            style: "blue".into(),
            bar_width: 6,
            format: "\u{1f4cb} [$bar] $done/$total".into(),
            thresholds: KanbanThresholdConfig::default(),
        }
    }
}

/// Kanban threshold configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct KanbanThresholdConfig {
    pub low: ThresholdEntry,
    pub medium: ThresholdEntry,
    pub high: ThresholdEntry,
}

impl Default for KanbanThresholdConfig {
    fn default() -> Self {
        Self {
            low: ThresholdEntry {
                below: 25,
                style: "red".into(),
            },
            medium: ThresholdEntry {
                below: 75,
                style: "yellow".into(),
            },
            high: ThresholdEntry {
                below: 101,
                style: "green".into(),
            },
        }
    }
}

/// Index module configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct IndexModuleConfig {
    pub style: String,
    pub format: String,
    pub show_when_complete: bool,
}

impl Default for IndexModuleConfig {
    fn default() -> Self {
        Self {
            style: "blue".into(),
            format: "\u{1f5c2} $percent%".into(),
            show_when_complete: false,
        }
    }
}

/// Languages module configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct LanguagesModuleConfig {
    pub style: String,
    pub dim_without_lsp: bool,
    /// Indicator appended to language icons when the LSP server is not found.
    /// Set to empty string to disable.
    pub missing_lsp_indicator: String,
    pub format: String,
}

impl Default for LanguagesModuleConfig {
    fn default() -> Self {
        Self {
            style: "bold".into(),
            dim_without_lsp: true,
            missing_lsp_indicator: "\u{26a0}".into(),
            format: "$icons".into(),
        }
    }
}

/// Load the statusline config with 3-layer stacking:
/// builtin -> user (~/.sah/statusline/config.yaml) -> project (.sah/statusline/config.yaml)
///
/// Overlay files are deep-merged into the base so that a user or project config
/// only needs to specify the fields they want to override. Unspecified fields
/// retain their values from the previous layer.
pub fn load_config() -> StatuslineConfig {
    let mut base: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(BUILTIN_CONFIG_YAML).expect("builtin config.yaml must parse");

    // User layer
    if let Some(home) = dirs::home_dir() {
        let user_path = home.join(".sah").join("statusline").join("config.yaml");
        if let Some(overlay) = load_yaml_value(&user_path) {
            deep_merge(&mut base, overlay);
        }
    }

    // Project layer
    let project_path = Path::new(".sah").join("statusline").join("config.yaml");
    if let Some(overlay) = load_yaml_value(&project_path) {
        deep_merge(&mut base, overlay);
    }

    serde_yaml_ng::from_value(base).expect("merged config must deserialize")
}

/// Load a YAML file as a raw Value for merging. Returns None if not found or invalid.
fn load_yaml_value(path: &Path) -> Option<serde_yaml_ng::Value> {
    let content = std::fs::read_to_string(path).ok()?;
    match serde_yaml_ng::from_str(&content) {
        Ok(val) => Some(val),
        Err(e) => {
            tracing::warn!("Failed to parse {}: {}", path.display(), e);
            None
        }
    }
}

/// Recursively merge `overlay` into `base`. Only mapping keys present in the
/// overlay are overwritten; all other keys in `base` are preserved.
fn deep_merge(base: &mut serde_yaml_ng::Value, overlay: serde_yaml_ng::Value) {
    match (base, overlay) {
        (serde_yaml_ng::Value::Mapping(base_map), serde_yaml_ng::Value::Mapping(overlay_map)) => {
            for (key, overlay_val) in overlay_map {
                if let Some(base_val) = base_map.get_mut(&key) {
                    deep_merge(base_val, overlay_val);
                } else {
                    base_map.insert(key, overlay_val);
                }
            }
        }
        (base, overlay) => {
            // Scalar or sequence: overlay replaces entirely
            *base = overlay;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deep_merge_overwrites_scalar() {
        let mut base: serde_yaml_ng::Value = serde_yaml_ng::from_str("format: old").unwrap();
        let overlay: serde_yaml_ng::Value = serde_yaml_ng::from_str("format: new").unwrap();
        deep_merge(&mut base, overlay);
        let result: StatuslineConfig = serde_yaml_ng::from_value(base).unwrap();
        assert_eq!(result.format, "new");
    }

    #[test]
    fn test_deep_merge_preserves_unspecified_fields() {
        let mut base: serde_yaml_ng::Value = serde_yaml_ng::from_str(BUILTIN_CONFIG_YAML).unwrap();
        let builtin: StatuslineConfig = serde_yaml_ng::from_str(BUILTIN_CONFIG_YAML).unwrap();

        // Overlay only changes directory style
        let overlay: serde_yaml_ng::Value =
            serde_yaml_ng::from_str("directory:\n  style: \"red bold\"").unwrap();
        deep_merge(&mut base, overlay);
        let result: StatuslineConfig = serde_yaml_ng::from_value(base).unwrap();

        // Changed field
        assert_eq!(result.directory.style, "red bold");
        // Preserved fields from builtin
        assert_eq!(
            result.directory.truncation_length,
            builtin.directory.truncation_length
        );
        assert_eq!(result.directory.format, builtin.directory.format);
        assert_eq!(result.format, builtin.format);
        assert_eq!(result.git_branch.style, builtin.git_branch.style);
    }

    #[test]
    fn test_deep_merge_nested_preserves_siblings() {
        let mut base: serde_yaml_ng::Value = serde_yaml_ng::from_str(BUILTIN_CONFIG_YAML).unwrap();
        let builtin: StatuslineConfig = serde_yaml_ng::from_str(BUILTIN_CONFIG_YAML).unwrap();

        // Override only git_status.show_counts
        let overlay: serde_yaml_ng::Value =
            serde_yaml_ng::from_str("git_status:\n  show_counts: true").unwrap();
        deep_merge(&mut base, overlay);
        let result: StatuslineConfig = serde_yaml_ng::from_value(base).unwrap();

        assert!(result.git_status.show_counts);
        // Sibling fields preserved
        assert_eq!(result.git_status.style, builtin.git_status.style);
        assert_eq!(result.git_status.modified, builtin.git_status.modified);
    }

    #[test]
    fn test_deep_merge_adds_new_keys() {
        let mut base: serde_yaml_ng::Value = serde_yaml_ng::from_str("a: 1").unwrap();
        let overlay: serde_yaml_ng::Value = serde_yaml_ng::from_str("b: 2").unwrap();
        deep_merge(&mut base, overlay);
        assert_eq!(base["a"], serde_yaml_ng::Value::Number(1.into()));
        assert_eq!(base["b"], serde_yaml_ng::Value::Number(2.into()));
    }

    #[test]
    fn test_load_config_returns_builtin_without_overrides() {
        // When no override files exist, load_config should return builtin values
        let config = load_config();
        let builtin: StatuslineConfig = serde_yaml_ng::from_str(BUILTIN_CONFIG_YAML).unwrap();
        assert_eq!(config.format, builtin.format);
        assert_eq!(config.directory.style, builtin.directory.style);
    }
}
