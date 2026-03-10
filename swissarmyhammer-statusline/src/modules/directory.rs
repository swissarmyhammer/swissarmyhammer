//! Directory module - shows the current working directory basename.

use std::collections::HashMap;
use std::path::Path;

use crate::module::{interpolate, ModuleContext, ModuleOutput};
use crate::style::Style;

/// Evaluate the directory module.
///
/// Shows the truncated current working directory from the Claude Code JSON input.
/// Falls back to `cwd` if `workspace.current_dir` is not present.
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    let cwd = ctx
        .input
        .workspace
        .as_ref()
        .and_then(|w| w.current_dir.as_deref())
        .or(ctx.input.cwd.as_deref());

    let path = match cwd {
        Some(p) => {
            let path = Path::new(p);
            let len = ctx.config.directory.truncation_length;
            let components: Vec<_> = path.components().collect();
            if len > 0 && components.len() > len {
                components[components.len() - len..]
                    .iter()
                    .map(|c| c.as_os_str().to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join("/")
            } else {
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| p.to_string())
            }
        }
        None => return ModuleOutput::hidden(),
    };

    let mut vars = HashMap::new();
    vars.insert("path".into(), path);
    let text = interpolate(&ctx.config.directory.format, &vars);
    ModuleOutput::new(text, Style::parse(&ctx.config.directory.style))
}
