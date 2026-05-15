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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statusline_input_default() {
        let input = StatuslineInput::default();
        assert!(input.workspace.is_none());
        assert!(input.model.is_none());
        assert!(input.context_window.is_none());
        assert!(input.cost.is_none());
        assert!(input.session_id.is_none());
        assert!(input.vim.is_none());
        assert!(input.agent.is_none());
        assert!(input.worktree.is_none());
        assert!(input.version.is_none());
        assert!(input.cwd.is_none());
    }

    #[test]
    fn test_statusline_input_full_deserialize() {
        let json = r#"{
            "workspace": {"current_dir": "/home"},
            "model": {"display_name": "Claude", "id": "c3"},
            "context_window": {"used_percentage": 50.0, "remaining_percentage": 50.0, "context_window_size": 200000},
            "cost": {"total_cost_usd": 1.23},
            "session_id": "abc123",
            "vim": {"mode": "NORMAL"},
            "agent": {"name": "explorer"},
            "worktree": {"branch": "main"},
            "version": "1.0.0",
            "cwd": "/tmp"
        }"#;
        let input: StatuslineInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.workspace.unwrap().current_dir.unwrap(), "/home");
        let model = input.model.unwrap();
        assert_eq!(model.display_name.unwrap(), "Claude");
        assert_eq!(model.id.unwrap(), "c3");
        let cw = input.context_window.unwrap();
        assert_eq!(cw.used_percentage.unwrap(), 50.0);
        assert_eq!(cw.remaining_percentage.unwrap(), 50.0);
        assert_eq!(cw.context_window_size.unwrap(), 200000);
        assert_eq!(input.cost.unwrap().total_cost_usd.unwrap(), 1.23);
        assert_eq!(input.session_id.unwrap(), "abc123");
        assert_eq!(input.vim.unwrap().mode.unwrap(), "NORMAL");
        assert_eq!(input.agent.unwrap().name.unwrap(), "explorer");
        assert_eq!(input.worktree.unwrap().branch.unwrap(), "main");
        assert_eq!(input.version.unwrap(), "1.0.0");
        assert_eq!(input.cwd.unwrap(), "/tmp");
    }

    #[test]
    fn test_statusline_input_partial_deserialize() {
        let json = r#"{"model": {"display_name": "Claude"}}"#;
        let input: StatuslineInput = serde_json::from_str(json).unwrap();
        assert!(input.model.is_some());
        assert!(input.workspace.is_none());
        assert!(input.cwd.is_none());
    }

    #[test]
    fn test_statusline_input_empty_json() {
        let input: StatuslineInput = serde_json::from_str("{}").unwrap();
        assert!(input.model.is_none());
    }

    #[test]
    fn test_workspace_info_default() {
        let w = WorkspaceInfo::default();
        assert!(w.current_dir.is_none());
    }

    #[test]
    fn test_model_info_default() {
        let m = ModelInfo::default();
        assert!(m.display_name.is_none());
        assert!(m.id.is_none());
    }

    #[test]
    fn test_context_window_info_default() {
        let c = ContextWindowInfo::default();
        assert!(c.used_percentage.is_none());
        assert!(c.remaining_percentage.is_none());
        assert!(c.context_window_size.is_none());
    }

    #[test]
    fn test_cost_info_default() {
        let c = CostInfo::default();
        assert!(c.total_cost_usd.is_none());
    }

    #[test]
    fn test_vim_info_default() {
        let v = VimInfo::default();
        assert!(v.mode.is_none());
    }

    #[test]
    fn test_agent_info_default() {
        let a = AgentInfo::default();
        assert!(a.name.is_none());
    }

    #[test]
    fn test_worktree_info_default() {
        let w = WorktreeInfo::default();
        assert!(w.branch.is_none());
    }

    #[test]
    fn test_statusline_input_unknown_fields_ignored() {
        let json = r#"{"unknown_field": "value", "model": {"display_name": "X"}}"#;
        let input: StatuslineInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.model.unwrap().display_name.unwrap(), "X");
    }

    #[test]
    fn test_debug_impls() {
        let input = StatuslineInput::default();
        let _ = format!("{:?}", input);
        let _ = format!("{:?}", WorkspaceInfo::default());
        let _ = format!("{:?}", ModelInfo::default());
        let _ = format!("{:?}", ContextWindowInfo::default());
        let _ = format!("{:?}", CostInfo::default());
        let _ = format!("{:?}", VimInfo::default());
        let _ = format!("{:?}", AgentInfo::default());
        let _ = format!("{:?}", WorktreeInfo::default());
    }
}
