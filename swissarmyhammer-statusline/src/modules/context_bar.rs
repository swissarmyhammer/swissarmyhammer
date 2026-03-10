//! Context bar module - shows context window usage as a progress bar.

use std::collections::HashMap;

use crate::module::{interpolate, ModuleContext, ModuleOutput};
use crate::style::Style;

/// Evaluate the context bar module.
///
/// Renders a progress bar showing context window usage percentage,
/// with color thresholds for low/medium/high usage.
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    let pct = match ctx
        .input
        .context_window
        .as_ref()
        .and_then(|c| c.used_percentage)
    {
        Some(p) => p,
        None => return ModuleOutput::hidden(),
    };

    let cfg = &ctx.config.context_bar;
    let width = cfg.bar_width;
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;

    let bar = format!(
        "{}{}",
        "\u{2588}".repeat(filled),
        "\u{2591}".repeat(empty)
    );

    let style_str = if (pct as u32) < cfg.thresholds.low.below {
        &cfg.thresholds.low.style
    } else if (pct as u32) < cfg.thresholds.medium.below {
        &cfg.thresholds.medium.style
    } else {
        &cfg.thresholds.high.style
    };

    let mut vars = HashMap::new();
    vars.insert("bar".into(), bar);
    vars.insert("percentage".into(), format!("{}", pct as u32));
    let text = interpolate(&cfg.format, &vars);
    ModuleOutput::new(text, Style::parse(style_str))
}
