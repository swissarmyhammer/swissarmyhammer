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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StatuslineConfig;
    use crate::input::{ModelInfo, StatuslineInput};

    #[test]
    fn test_model_display_name() {
        let input = StatuslineInput {
            model: Some(ModelInfo {
                display_name: Some("Claude".into()),
                id: None,
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
        assert!(out.text.contains("Claude"));
    }

    #[test]
    fn test_model_fallback_to_id() {
        let input = StatuslineInput {
            model: Some(ModelInfo {
                display_name: None,
                id: Some("claude-3".into()),
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
        assert!(out.text.contains("claude-3"));
    }

    #[test]
    fn test_model_none() {
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
    fn test_model_both_none() {
        let input = StatuslineInput {
            model: Some(ModelInfo {
                display_name: None,
                id: None,
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
    fn test_model_render_output() {
        let input = StatuslineInput {
            model: Some(ModelInfo {
                display_name: Some("Claude".into()),
                id: None,
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
        assert!(rendered.contains("Claude"));
        assert!(rendered.contains("\x1b["));
    }

    #[test]
    fn test_model_prefers_display_name_over_id() {
        let input = StatuslineInput {
            model: Some(ModelInfo {
                display_name: Some("Display".into()),
                id: Some("model-id".into()),
            }),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(out.text.contains("Display"));
        assert!(!out.text.contains("model-id"));
    }
}
