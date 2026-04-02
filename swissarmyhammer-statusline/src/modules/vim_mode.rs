//! Vim mode module - shows the current vim mode.

use std::collections::HashMap;

use crate::module::{interpolate, ModuleContext, ModuleOutput};
use crate::style::Style;

/// Evaluate the vim mode module.
///
/// Shows the current vim mode string. Hidden when vim mode is not active.
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    let mode = match ctx.input.vim.as_ref().and_then(|v| v.mode.as_deref()) {
        Some(m) => m,
        None => return ModuleOutput::hidden(),
    };

    let mut vars = HashMap::new();
    vars.insert("mode".into(), mode.to_string());
    let text = interpolate(&ctx.config.vim_mode.format, &vars);
    ModuleOutput::new(text, Style::parse(&ctx.config.vim_mode.style))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StatuslineConfig;
    use crate::input::{StatuslineInput, VimInfo};

    #[test]
    fn test_vim_mode_present() {
        let input = StatuslineInput {
            vim: Some(VimInfo {
                mode: Some("NORMAL".into()),
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
        assert!(out.text.contains("NORMAL"));
    }

    #[test]
    fn test_vim_mode_none() {
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
    fn test_vim_mode_no_mode() {
        let input = StatuslineInput {
            vim: Some(VimInfo { mode: None }),
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
}
