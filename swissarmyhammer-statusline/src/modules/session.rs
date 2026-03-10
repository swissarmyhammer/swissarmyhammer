//! Session module - shows a truncated session ID.

use std::collections::HashMap;

use crate::module::{interpolate, ModuleContext, ModuleOutput};
use crate::style::Style;

/// Evaluate the session module.
///
/// Shows the first 8 characters of the session ID.
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    let id = match ctx.input.session_id.as_deref() {
        Some(id) => &id[..id.len().min(8)],
        None => return ModuleOutput::hidden(),
    };

    let mut vars = HashMap::new();
    vars.insert("id".into(), id.to_string());
    let text = interpolate(&ctx.config.session.format, &vars);
    ModuleOutput::new(text, Style::parse(&ctx.config.session.style))
}
