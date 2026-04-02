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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StatuslineConfig;
    use crate::input::{StatuslineInput, WorkspaceInfo};

    #[test]
    fn test_directory_from_workspace() {
        let input = StatuslineInput {
            workspace: Some(WorkspaceInfo {
                current_dir: Some("/home/user/project".into()),
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
        assert!(out.text.contains("project"));
    }

    #[test]
    fn test_directory_from_cwd_fallback() {
        let input = StatuslineInput {
            cwd: Some("/tmp/mydir".into()),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(!out.is_empty());
        assert!(out.text.contains("mydir"));
    }

    #[test]
    fn test_directory_none() {
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
    fn test_directory_truncation() {
        let input = StatuslineInput {
            cwd: Some("/home/user/deep/nested/project".into()),
            ..Default::default()
        };
        let mut config = StatuslineConfig::default();
        config.directory.truncation_length = 2;
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(out.text.contains("nested/project"));
    }

    #[test]
    fn test_directory_no_truncation() {
        let input = StatuslineInput {
            cwd: Some("/home/user/project".into()),
            ..Default::default()
        };
        let mut config = StatuslineConfig::default();
        config.directory.truncation_length = 0;
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(out.text.contains("project"));
    }

    #[test]
    fn test_directory_root_path() {
        let input = StatuslineInput {
            cwd: Some("/".into()),
            ..Default::default()
        };
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        assert!(!out.is_empty());
    }
}
