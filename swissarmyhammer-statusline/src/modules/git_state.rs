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
    render_state(state, ctx)
}

/// Map a git repository state to a human-readable label.
/// Returns None for Clean state.
fn state_label(state: git2::RepositoryState) -> Option<&'static str> {
    match state {
        git2::RepositoryState::Rebase
        | git2::RepositoryState::RebaseInteractive
        | git2::RepositoryState::RebaseMerge => Some("REBASING"),
        git2::RepositoryState::Merge => Some("MERGING"),
        git2::RepositoryState::CherryPick | git2::RepositoryState::CherryPickSequence => {
            Some("CHERRY-PICKING")
        }
        git2::RepositoryState::Revert | git2::RepositoryState::RevertSequence => Some("REVERTING"),
        git2::RepositoryState::Bisect => Some("BISECTING"),
        git2::RepositoryState::ApplyMailbox | git2::RepositoryState::ApplyMailboxOrRebase => {
            Some("APPLYING")
        }
        git2::RepositoryState::Clean => None,
    }
}

/// Render a git state label into styled module output.
fn render_state(state: git2::RepositoryState, ctx: &ModuleContext) -> ModuleOutput {
    let state_str = match state_label(state) {
        Some(s) => s,
        None => return ModuleOutput::hidden(),
    };

    let cfg = &ctx.config.git_state;
    let mut vars = HashMap::new();
    vars.insert("state".into(), state_str.to_string());
    vars.insert("progress".into(), String::new());
    let text = interpolate(&cfg.format, &vars);
    ModuleOutput::new(text, Style::parse(&cfg.style))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StatuslineConfig;
    use crate::input::StatuslineInput;

    // Note: `eval()` discovers the git repo at the process-wide current directory,
    // so it can't be exercised safely from a unit test without CWD-changing machinery.
    // The "hidden when clean" contract is covered here by `test_render_state_clean_hidden`
    // (pure `render_state` input) and, for the `eval()` path, by
    // `tests/isolated_dir_tests.rs::test_git_state_no_repo` (tempdir + CWD guard).

    #[test]
    fn test_state_label_clean() {
        assert!(state_label(git2::RepositoryState::Clean).is_none());
    }

    #[test]
    fn test_state_label_rebase() {
        assert_eq!(state_label(git2::RepositoryState::Rebase), Some("REBASING"));
        assert_eq!(
            state_label(git2::RepositoryState::RebaseInteractive),
            Some("REBASING")
        );
        assert_eq!(
            state_label(git2::RepositoryState::RebaseMerge),
            Some("REBASING")
        );
    }

    #[test]
    fn test_state_label_merge() {
        assert_eq!(state_label(git2::RepositoryState::Merge), Some("MERGING"));
    }

    #[test]
    fn test_state_label_cherry_pick() {
        assert_eq!(
            state_label(git2::RepositoryState::CherryPick),
            Some("CHERRY-PICKING")
        );
        assert_eq!(
            state_label(git2::RepositoryState::CherryPickSequence),
            Some("CHERRY-PICKING")
        );
    }

    #[test]
    fn test_state_label_revert() {
        assert_eq!(
            state_label(git2::RepositoryState::Revert),
            Some("REVERTING")
        );
        assert_eq!(
            state_label(git2::RepositoryState::RevertSequence),
            Some("REVERTING")
        );
    }

    #[test]
    fn test_state_label_bisect() {
        assert_eq!(
            state_label(git2::RepositoryState::Bisect),
            Some("BISECTING")
        );
    }

    #[test]
    fn test_state_label_apply() {
        assert_eq!(
            state_label(git2::RepositoryState::ApplyMailbox),
            Some("APPLYING")
        );
        assert_eq!(
            state_label(git2::RepositoryState::ApplyMailboxOrRebase),
            Some("APPLYING")
        );
    }

    #[test]
    fn test_render_state_rebase() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = render_state(git2::RepositoryState::Rebase, &ctx);
        assert!(!out.is_empty());
        assert!(out.text.contains("REBASING"));
    }

    #[test]
    fn test_render_state_clean_hidden() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = render_state(git2::RepositoryState::Clean, &ctx);
        assert!(out.is_empty());
    }

    #[test]
    fn test_render_state_merge() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = render_state(git2::RepositoryState::Merge, &ctx);
        assert!(!out.is_empty());
        assert!(out.text.contains("MERGING"));
    }

    #[test]
    fn test_render_state_cherry_pick() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = render_state(git2::RepositoryState::CherryPick, &ctx);
        assert!(out.text.contains("CHERRY-PICKING"));
    }

    #[test]
    fn test_render_state_revert() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = render_state(git2::RepositoryState::Revert, &ctx);
        assert!(out.text.contains("REVERTING"));
    }

    #[test]
    fn test_render_state_bisect() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = render_state(git2::RepositoryState::Bisect, &ctx);
        assert!(out.text.contains("BISECTING"));
    }

    #[test]
    fn test_render_state_apply_mailbox() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = render_state(git2::RepositoryState::ApplyMailbox, &ctx);
        assert!(out.text.contains("APPLYING"));
    }

    #[test]
    fn test_render_state_output_has_style() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = render_state(git2::RepositoryState::Rebase, &ctx);
        let rendered = out.render();
        assert!(rendered.contains("\x1b["));
        assert!(rendered.contains("REBASING"));
    }
}
