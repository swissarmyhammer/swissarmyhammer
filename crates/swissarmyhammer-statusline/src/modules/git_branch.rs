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
    format_branch(&branch, ctx)
}

/// Format the branch name with truncation and styling.
fn format_branch(branch: &str, ctx: &ModuleContext) -> ModuleOutput {
    let cfg = &ctx.config.git_branch;

    let truncated = if branch.len() > cfg.truncation_length && cfg.truncation_length > 0 {
        format!(
            "{}{}",
            &branch[..cfg.truncation_length],
            cfg.truncation_symbol
        )
    } else {
        branch.to_string()
    };

    let mut vars = HashMap::new();
    vars.insert("symbol".into(), cfg.symbol.clone());
    vars.insert("branch".into(), truncated);
    let text = interpolate(&cfg.format, &vars);
    ModuleOutput::new(text, Style::parse(&cfg.style))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StatuslineConfig;
    use crate::input::StatuslineInput;

    #[test]
    fn test_git_branch_in_repo() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        // We're in a git repo, so this should produce output
        assert!(!out.is_empty());
    }

    #[test]
    fn test_git_branch_truncation() {
        let input = StatuslineInput::default();
        let mut config = StatuslineConfig::default();
        config.git_branch.truncation_length = 3;
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(!out.is_empty());
    }

    #[test]
    fn test_git_branch_no_truncation() {
        let input = StatuslineInput::default();
        let mut config = StatuslineConfig::default();
        config.git_branch.truncation_length = 0;
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(!out.is_empty());
    }

    #[test]
    fn test_format_branch_short() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = format_branch("main", &ctx);
        assert!(out.text.contains("main"));
    }

    #[test]
    fn test_format_branch_long_truncated() {
        let input = StatuslineInput::default();
        let mut config = StatuslineConfig::default();
        config.git_branch.truncation_length = 5;
        config.git_branch.truncation_symbol = "…".into();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = format_branch("feature/very-long-branch-name", &ctx);
        assert!(out.text.contains("featu"));
        assert!(out.text.contains("…"));
    }

    #[test]
    fn test_format_branch_zero_truncation() {
        let input = StatuslineInput::default();
        let mut config = StatuslineConfig::default();
        config.git_branch.truncation_length = 0;
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = format_branch("main", &ctx);
        assert!(out.text.contains("main"));
    }

    #[test]
    fn test_format_branch_render_output() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = format_branch("main", &ctx);
        let rendered = out.render();
        assert!(rendered.contains("main"));
        assert!(rendered.contains("\x1b["));
    }

    #[test]
    fn test_format_branch_exact_length() {
        let input = StatuslineInput::default();
        let mut config = StatuslineConfig::default();
        config.git_branch.truncation_length = 4;
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        // Branch name exactly at truncation length
        let out = format_branch("main", &ctx);
        assert!(out.text.contains("main"));
        // Should not contain truncation symbol since len == truncation_length
        assert!(!out.text.contains(&config.git_branch.truncation_symbol));
    }

    #[test]
    fn test_format_branch_one_over_truncation() {
        let input = StatuslineInput::default();
        let mut config = StatuslineConfig::default();
        config.git_branch.truncation_length = 4;
        config.git_branch.truncation_symbol = "...".into();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = format_branch("mains", &ctx);
        assert!(out.text.contains("main"));
        assert!(out.text.contains("..."));
    }
}
