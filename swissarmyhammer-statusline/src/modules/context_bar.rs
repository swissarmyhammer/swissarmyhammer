//! Context bar module - shows context window usage as a progress bar.

use std::collections::HashMap;

use crate::module::{interpolate, ModuleContext, ModuleOutput};
use crate::style::Style;

/// Evaluate the context bar module.
///
/// Renders a progress bar showing context window usage percentage,
/// with color thresholds for low/medium/high usage.
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    let pct = ctx
        .input
        .context_window
        .as_ref()
        .and_then(|c| c.used_percentage)
        .unwrap_or(0.0);

    let cfg = &ctx.config.context_bar;
    let width = cfg.bar_width;
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;

    let bar = format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(empty));

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StatuslineConfig;
    use crate::input::{ContextWindowInfo, StatuslineInput};

    #[test]
    fn test_context_bar_low_usage() {
        let input = StatuslineInput {
            context_window: Some(ContextWindowInfo {
                used_percentage: Some(25.0),
                ..Default::default()
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
        assert!(out.text.contains("25%"));
    }

    #[test]
    fn test_context_bar_medium_usage() {
        let input = StatuslineInput {
            context_window: Some(ContextWindowInfo {
                used_percentage: Some(65.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(out.text.contains("65%"));
    }

    #[test]
    fn test_context_bar_high_usage() {
        let input = StatuslineInput {
            context_window: Some(ContextWindowInfo {
                used_percentage: Some(90.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(out.text.contains("90%"));
    }

    #[test]
    fn test_context_bar_no_data() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(out.text.contains("0%"));
    }

    #[test]
    fn test_context_bar_100_percent() {
        let input = StatuslineInput {
            context_window: Some(ContextWindowInfo {
                used_percentage: Some(100.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(out.text.contains("100%"));
    }
}
