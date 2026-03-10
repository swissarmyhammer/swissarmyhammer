//! Git branch module - shows the current git branch name.

use std::collections::HashMap;

use crate::module::{interpolate, ModuleContext, ModuleOutput};
use crate::style::Style;

/// Evaluate the git branch module.
///
/// Uses git2 to discover the repository and read the current HEAD branch.
/// Truncates long branch names according to config.
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    let repo = match git2::Repository::discover(".") {
        Ok(r) => r,
        Err(_) => return ModuleOutput::hidden(),
    };

    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return ModuleOutput::hidden(),
    };

    let branch = head.shorthand().unwrap_or("HEAD").to_string();
    let cfg = &ctx.config.git_branch;

    let truncated = if branch.len() > cfg.truncation_length && cfg.truncation_length > 0 {
        format!(
            "{}{}",
            &branch[..cfg.truncation_length],
            cfg.truncation_symbol
        )
    } else {
        branch
    };

    let mut vars = HashMap::new();
    vars.insert("symbol".into(), cfg.symbol.clone());
    vars.insert("branch".into(), truncated);
    let text = interpolate(&cfg.format, &vars);
    ModuleOutput::new(text, Style::parse(&cfg.style))
}
