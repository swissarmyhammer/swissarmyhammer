//! Model module - shows the current AI model name.

use std::collections::HashMap;

use crate::module::{interpolate, ModuleContext, ModuleOutput};
use crate::style::Style;

/// Evaluate the model module.
///
/// Shows the model display name or ID from the Claude Code JSON input.
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    let name = ctx
        .input
        .model
        .as_ref()
        .and_then(|m| m.display_name.as_deref().or(m.id.as_deref()));

    match name {
        Some(name) => {
            let mut vars = HashMap::new();
            vars.insert("name".into(), name.to_string());
            let text = interpolate(&ctx.config.model.format, &vars);
            ModuleOutput::new(text, Style::parse(&ctx.config.model.style))
        }
        None => ModuleOutput::hidden(),
    }
}
