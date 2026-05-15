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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StatuslineConfig;
    use crate::input::{CostInfo, StatuslineInput};

    #[test]
    fn test_cost_present() {
        let input = StatuslineInput {
            cost: Some(CostInfo {
                total_cost_usd: Some(1.50),
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
        assert!(out.text.contains("1.50"));
    }

    #[test]
    fn test_cost_none() {
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
    fn test_cost_zero_hidden() {
        let input = StatuslineInput {
            cost: Some(CostInfo {
                total_cost_usd: Some(0.0),
            }),
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

    #[test]
    fn test_cost_zero_shown_when_hide_zero_disabled() {
        let input = StatuslineInput {
            cost: Some(CostInfo {
                total_cost_usd: Some(0.0),
            }),
            ..Default::default()
        };
        let mut config = StatuslineConfig::default();
        config.cost.hide_zero = false;
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(!out.is_empty());
        assert!(out.text.contains("0.00"));
    }

    #[test]
    fn test_cost_none_amount() {
        let input = StatuslineInput {
            cost: Some(CostInfo {
                total_cost_usd: None,
            }),
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

    #[test]
    fn test_cost_render_output() {
        let input = StatuslineInput {
            cost: Some(CostInfo {
                total_cost_usd: Some(5.99),
            }),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        let rendered = out.render();
        assert!(rendered.contains("5.99"));
    }

    #[test]
    fn test_cost_small_nonzero() {
        let input = StatuslineInput {
            cost: Some(CostInfo {
                total_cost_usd: Some(0.01),
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
        assert!(out.text.contains("0.01"));
    }

    #[test]
    fn test_cost_near_zero_hidden() {
        let input = StatuslineInput {
            cost: Some(CostInfo {
                total_cost_usd: Some(0.004),
            }),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        // 0.004 < 0.005 threshold with hide_zero = true
        assert!(out.is_empty());
    }
}
