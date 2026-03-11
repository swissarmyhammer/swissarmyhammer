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
    /// This must NOT call `serde_yaml::from_str` because `StatuslineConfig` has
    /// `#[serde(default)]` on the struct, which means serde calls `Self::default()`
    /// during deserialization for any missing fields. If `default()` itself called
    /// `serde_yaml::from_str`, that would trigger infinite recursion and a stack
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
            style: "yellow".into(),
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
/// builtin -> user (~/.swissarmyhammer/statusline/config.yaml) -> project (.swissarmyhammer/statusline/config.yaml)
pub fn load_config() -> StatuslineConfig {
    // Parse from the builtin YAML rather than using Default::default() directly.
    // This is safe from recursion because Default::default() no longer calls serde.
    let mut config: StatuslineConfig =
        serde_yaml::from_str(BUILTIN_CONFIG_YAML).expect("builtin config.yaml must parse");

    // User layer
    if let Some(home) = dirs::home_dir() {
        let user_path = home
            .join(".swissarmyhammer")
            .join("statusline")
            .join("config.yaml");
        if let Some(overlay) = load_yaml_file(&user_path) {
            config = overlay;
        }
    }

    // Project layer
    let project_path = Path::new(".swissarmyhammer")
        .join("statusline")
        .join("config.yaml");
    if let Some(overlay) = load_yaml_file(&project_path) {
        config = overlay;
    }

    config
}

/// Load and parse a YAML config file, returning None if not found or invalid.
fn load_yaml_file(path: &Path) -> Option<StatuslineConfig> {
    let content = std::fs::read_to_string(path).ok()?;
    match serde_yaml::from_str(&content) {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            tracing::warn!("Failed to parse {}: {}", path.display(), e);
            None
        }
    }
}
