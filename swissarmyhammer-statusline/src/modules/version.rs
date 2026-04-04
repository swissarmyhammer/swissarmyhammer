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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StatuslineConfig;
    use crate::input::StatuslineInput;

    #[test]
    fn test_version_present() {
        let input = StatuslineInput {
            version: Some("1.2.3".into()),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(!out.is_empty());
        assert!(out.text.contains("1.2.3"));
    }

    #[test]
    fn test_version_none() {
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
    fn test_version_render_output() {
        let input = StatuslineInput {
            version: Some("1.2.3".into()),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        let rendered = out.render();
        assert!(rendered.contains("v1.2.3"));
    }

    #[test]
    fn test_version_empty_string() {
        let input = StatuslineInput {
            version: Some("".into()),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(out.text.contains("v"));
    }
}
