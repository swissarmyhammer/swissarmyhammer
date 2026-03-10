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
