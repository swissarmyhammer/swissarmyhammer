//! Git state module - shows repository state during rebase, merge, etc.

use std::collections::HashMap;

use crate::module::{interpolate, ModuleContext, ModuleOutput};
use crate::style::Style;

/// Evaluate the git state module.
///
/// Shows the repository state (REBASING, MERGING, CHERRY-PICKING, etc.)
/// when the repository is in the middle of an operation. Hidden when clean.
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    let repo = match git2::Repository::discover(".") {
        Ok(r) => r,
        Err(_) => return ModuleOutput::hidden(),
    };

    let state = repo.state();
    let state_str = match state {
        git2::RepositoryState::Rebase
        | git2::RepositoryState::RebaseInteractive
        | git2::RepositoryState::RebaseMerge => "REBASING",
        git2::RepositoryState::Merge => "MERGING",
        git2::RepositoryState::CherryPick | git2::RepositoryState::CherryPickSequence => {
            "CHERRY-PICKING"
        }
        git2::RepositoryState::Revert | git2::RepositoryState::RevertSequence => "REVERTING",
        git2::RepositoryState::Bisect => "BISECTING",
        git2::RepositoryState::ApplyMailbox | git2::RepositoryState::ApplyMailboxOrRebase => {
            "APPLYING"
        }
        git2::RepositoryState::Clean => return ModuleOutput::hidden(),
    };

    let cfg = &ctx.config.git_state;
    let mut vars = HashMap::new();
    vars.insert("state".into(), state_str.to_string());
    vars.insert("progress".into(), String::new()); // TODO: parse rebase progress
    let text = interpolate(&cfg.format, &vars);
    ModuleOutput::new(text, Style::parse(&cfg.style))
}
