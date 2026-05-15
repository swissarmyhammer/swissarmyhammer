//! Worktree module - shows the worktree branch.

use std::collections::HashMap;

use crate::module::{interpolate, ModuleContext, ModuleOutput};
use crate::style::Style;

/// Evaluate the worktree module.
///
/// Shows the worktree branch name. Hidden when not in a worktree.
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    let branch = match ctx
        .input
        .worktree
        .as_ref()
        .and_then(|w| w.branch.as_deref())
    {
        Some(b) => b,
        None => return ModuleOutput::hidden(),
    };

    let mut vars = HashMap::new();
    vars.insert("branch".into(), branch.to_string());
    let text = interpolate(&ctx.config.worktree.format, &vars);
    ModuleOutput::new(text, Style::parse(&ctx.config.worktree.style))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StatuslineConfig;
    use crate::input::{StatuslineInput, WorktreeInfo};

    #[test]
    fn test_worktree_present() {
        let input = StatuslineInput {
            worktree: Some(WorktreeInfo {
                branch: Some("feature-123".into()),
            }),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(!out.is_empty());
        assert!(out.text.contains("feature-123"));
    }

    #[test]
    fn test_worktree_none() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(out.is_empty());
    }

    #[test]
    fn test_worktree_no_branch() {
        let input = StatuslineInput {
            worktree: Some(WorktreeInfo { branch: None }),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(out.is_empty());
    }

    #[test]
    fn test_worktree_render_output() {
        let input = StatuslineInput {
            worktree: Some(WorktreeInfo {
                branch: Some("develop".into()),
            }),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        let rendered = out.render();
        assert!(rendered.contains("develop"));
        assert!(rendered.contains("\x1b["));
    }
}
