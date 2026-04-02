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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StatuslineConfig;
    use crate::input::{AgentInfo, StatuslineInput};

    #[test]
    fn test_agent_present() {
        let input = StatuslineInput {
            agent: Some(AgentInfo {
                name: Some("explorer".into()),
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
        assert!(out.text.contains("explorer"));
    }

    #[test]
    fn test_agent_none() {
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
    fn test_agent_with_no_name() {
        let input = StatuslineInput {
            agent: Some(AgentInfo { name: None }),
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
}
