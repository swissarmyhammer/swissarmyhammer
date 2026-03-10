//! Serde structs for Claude Code statusline JSON input.

use serde::Deserialize;

/// Top-level input from Claude Code's statusline JSON.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StatuslineInput {
    pub workspace: Option<WorkspaceInfo>,
    pub model: Option<ModelInfo>,
    pub context_window: Option<ContextWindowInfo>,
    pub cost: Option<CostInfo>,
    pub session_id: Option<String>,
    pub vim: Option<VimInfo>,
    pub agent: Option<AgentInfo>,
    pub worktree: Option<WorktreeInfo>,
    pub version: Option<String>,
    /// Fallback field for cwd
    pub cwd: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct WorkspaceInfo {
    pub current_dir: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ModelInfo {
    pub display_name: Option<String>,
    pub id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ContextWindowInfo {
    pub used_percentage: Option<f64>,
    pub remaining_percentage: Option<f64>,
    pub context_window_size: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct CostInfo {
    pub total_cost_usd: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct VimInfo {
    pub mode: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct AgentInfo {
    pub name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct WorktreeInfo {
    pub branch: Option<String>,
}
