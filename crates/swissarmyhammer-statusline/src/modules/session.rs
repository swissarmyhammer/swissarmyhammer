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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StatuslineConfig;
    use crate::input::StatuslineInput;

    #[test]
    fn test_session_present() {
        let input = StatuslineInput {
            session_id: Some("abcdef1234567890".into()),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(!out.is_empty());
        assert!(out.text.contains("abcdef12"));
    }

    #[test]
    fn test_session_short_id() {
        let input = StatuslineInput {
            session_id: Some("abc".into()),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(out.text.contains("abc"));
    }

    #[test]
    fn test_session_none() {
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
    fn test_session_render_output() {
        let input = StatuslineInput {
            session_id: Some("abcdef1234567890".into()),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        let rendered = out.render();
        assert!(rendered.contains("abcdef12"));
    }

    #[test]
    fn test_session_exact_8_chars() {
        let input = StatuslineInput {
            session_id: Some("12345678".into()),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(out.text.contains("12345678"));
    }

    #[test]
    fn test_session_empty_string() {
        let input = StatuslineInput {
            session_id: Some("".into()),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        // Empty session_id truncated to "" which interpolates to empty format output
        let _ = out.is_empty();
        let _ = out.render();
    }
}
