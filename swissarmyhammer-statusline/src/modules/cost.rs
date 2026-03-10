//! Cost module - shows the total session cost in USD.

use std::collections::HashMap;

use crate::module::{interpolate, ModuleContext, ModuleOutput};
use crate::style::Style;

/// Evaluate the cost module.
///
/// Shows total session cost formatted as USD. Hidden when cost is zero
/// and `hide_zero` is enabled.
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    let amount = match ctx.input.cost.as_ref().and_then(|c| c.total_cost_usd) {
        Some(a) => a,
        None => return ModuleOutput::hidden(),
    };

    if ctx.config.cost.hide_zero && amount < 0.005 {
        return ModuleOutput::hidden();
    }

    let mut vars = HashMap::new();
    vars.insert("amount".into(), format!("{:.2}", amount));
    let text = interpolate(&ctx.config.cost.format, &vars);
    ModuleOutput::new(text, Style::parse(&ctx.config.cost.style))
}
