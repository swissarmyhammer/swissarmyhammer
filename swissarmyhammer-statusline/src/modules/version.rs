//! Version module - shows the Claude Code version.

use std::collections::HashMap;

use crate::module::{interpolate, ModuleContext, ModuleOutput};
use crate::style::Style;

/// Evaluate the version module.
///
/// Shows the Claude Code version string.
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    let version = match ctx.input.version.as_deref() {
        Some(v) => v,
        None => return ModuleOutput::hidden(),
    };

    let mut vars = HashMap::new();
    vars.insert("version".into(), version.to_string());
    let text = interpolate(&ctx.config.version.format, &vars);
    ModuleOutput::new(text, Style::parse(&ctx.config.version.style))
}
