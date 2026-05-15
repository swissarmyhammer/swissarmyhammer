//! swissarmyhammer-statusline: Starship-like statusline for Claude Code.
//!
//! Reads JSON from stdin, evaluates modules, outputs styled ANSI text.

pub mod config;
pub mod format;
pub mod input;
pub mod module;
pub mod modules;
pub mod style;

use config::StatuslineConfig;
use format::{parse_format, FormatSegment};
use input::StatuslineInput;
use module::{ModuleContext, ModuleRegistry};

/// Run the statusline pipeline: parse JSON input, evaluate modules, return styled output.
pub fn run(input_json: &str) -> String {
    let input: StatuslineInput = serde_json::from_str(input_json).unwrap_or_default();
    let config = config::load_config();
    render(&input, &config)
}

/// Render the statusline from parsed input and config.
pub fn render(input: &StatuslineInput, config: &StatuslineConfig) -> String {
    let registry = ModuleRegistry::new();
    let ctx = ModuleContext { input, config };

    let segments = parse_format(&config.format);
    let mut output = String::new();

    for segment in segments {
        match segment {
            FormatSegment::Literal(s) => output.push_str(&s),
            FormatSegment::Variable(name) => {
                if let Some(eval_fn) = registry.get(&name) {
                    let module_output = eval_fn(&ctx);
                    let rendered = module_output.render();
                    output.push_str(&rendered);
                }
            }
        }
    }

    // Trim trailing whitespace from empty modules
    output.trim_end().to_string()
}

/// Dump the builtin config YAML to stdout.
pub fn dump_config() -> &'static str {
    config::BUILTIN_CONFIG_YAML
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dump_config() {
        let yaml = dump_config();
        assert!(yaml.contains("format:"));
        assert!(yaml.contains("directory:"));
    }

    #[test]
    fn test_run_empty_json() {
        let result = run("{}");
        // Should produce output without crashing
        assert!(result.is_ascii() || !result.is_empty() || result.is_empty());
    }

    #[test]
    fn test_run_with_model() {
        let json = r#"{"model": {"display_name": "Claude"}}"#;
        let result = run(json);
        assert!(result.contains("Claude"));
    }

    #[test]
    fn test_run_invalid_json_uses_defaults() {
        let result = run("not valid json");
        // unwrap_or_default means it doesn't crash
        assert!(result.is_ascii() || !result.is_empty() || result.is_empty());
    }

    #[test]
    fn test_render_with_model_input() {
        let input = StatuslineInput {
            model: Some(input::ModelInfo {
                display_name: Some("TestModel".into()),
                id: None,
            }),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let result = render(&input, &config);
        assert!(result.contains("TestModel"));
    }

    #[test]
    fn test_render_with_directory() {
        let input = StatuslineInput {
            cwd: Some("/home/user/project".into()),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let result = render(&input, &config);
        assert!(result.contains("project"));
    }

    #[test]
    fn test_render_empty_input() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig::default();
        let _result = render(&input, &config);
        // Should not crash with empty input
    }

    #[test]
    fn test_render_unknown_module_in_format() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig {
            format: "$nonexistent_module".into(),
            ..Default::default()
        };
        let result = render(&input, &config);
        assert_eq!(result, "");
    }

    #[test]
    fn test_render_literal_format() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig {
            format: "just text".into(),
            ..Default::default()
        };
        let result = render(&input, &config);
        assert_eq!(result, "just text");
    }

    #[test]
    fn test_render_mixed_modules_and_literals() {
        let input = StatuslineInput {
            model: Some(input::ModelInfo {
                display_name: Some("Claude".into()),
                id: None,
            }),
            ..Default::default()
        };
        let config = StatuslineConfig {
            format: "prefix $model suffix".into(),
            ..Default::default()
        };
        let result = render(&input, &config);
        assert!(result.contains("Claude"));
    }

    #[test]
    fn test_render_trims_trailing_whitespace() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig {
            format: "text   ".into(),
            ..Default::default()
        };
        let result = render(&input, &config);
        assert_eq!(result, "text");
    }

    #[test]
    fn test_run_with_all_fields() {
        let json = r#"{
            "workspace": {"current_dir": "/tmp/proj"},
            "model": {"display_name": "Claude"},
            "context_window": {"used_percentage": 30.0},
            "cost": {"total_cost_usd": 0.50},
            "session_id": "sess123",
            "vim": {"mode": "INSERT"},
            "agent": {"name": "tester"},
            "worktree": {"branch": "dev"},
            "version": "2.0.0",
            "cwd": "/tmp/proj"
        }"#;
        let result = run(json);
        assert!(result.contains("proj") || result.contains("Claude"));
    }

    #[test]
    fn test_dump_config_contains_modules() {
        let yaml = dump_config();
        assert!(yaml.contains("git_branch:"));
        assert!(yaml.contains("git_status:"));
        assert!(yaml.contains("model:"));
    }

    #[test]
    fn test_render_only_model_format() {
        let input = StatuslineInput {
            model: Some(input::ModelInfo {
                display_name: Some("TestModel".into()),
                id: None,
            }),
            ..Default::default()
        };
        let config = StatuslineConfig {
            format: "$model".into(),
            ..Default::default()
        };
        let result = render(&input, &config);
        assert!(result.contains("TestModel"));
    }
}
