//! Agent module - shows the current agent name.

use std::collections::HashMap;

use crate::module::{interpolate, ModuleContext, ModuleOutput};
use crate::style::Style;

/// Evaluate the agent module.
///
/// Shows the current agent name. Hidden when no agent is active.
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    let name = match ctx.input.agent.as_ref().and_then(|a| a.name.as_deref()) {
        Some(n) => n,
        None => return ModuleOutput::hidden(),
    };

    let mut vars = HashMap::new();
    vars.insert("name".into(), name.to_string());
    let text = interpolate(&ctx.config.agent.format, &vars);
    ModuleOutput::new(text, Style::parse(&ctx.config.agent.style))
}
