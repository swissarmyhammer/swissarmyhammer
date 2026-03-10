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
