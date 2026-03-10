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
