//! Index module - shows code-context indexing progress and LSP health.

use std::collections::HashMap;

use crate::module::{interpolate, ModuleContext, ModuleOutput};
use crate::style::Style;

/// Evaluate the index module.
///
/// Shows the code-context indexing progress percentage and a `/lsp to fix`
/// prompt when LSP servers are missing but have install hints. Hidden when
/// indexing is complete and no fixable LSPs are missing (unless
/// `show_when_complete` is set).
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    try_eval(ctx).unwrap_or_else(ModuleOutput::hidden)
}

fn try_eval(ctx: &ModuleContext) -> Option<ModuleOutput> {
    let cwd = std::env::current_dir().ok()?;
    let ws = swissarmyhammer_code_context::CodeContextWorkspace::open(&cwd).ok()?;
    let conn = ws.db();
    let status = swissarmyhammer_code_context::get_status(&conn).ok()?;

    let cfg = &ctx.config.index;
    let percent = status.ts_indexed_percent as u32;
    let has_fixable_missing = has_fixable_missing_lsps(&conn);

    Some(render_index(
        percent,
        status.total_files,
        status.dirty_files,
        status.ts_chunk_count,
        status.lsp_symbol_count,
        has_fixable_missing,
        cfg,
    ))
}

/// Render the index module output from pre-computed values.
fn render_index(
    percent: u32,
    total_files: u64,
    dirty_files: u64,
    chunks: u64,
    symbols: u64,
    has_fixable_missing: bool,
    cfg: &crate::config::IndexModuleConfig,
) -> ModuleOutput {
    // Hide when indexing is complete AND no fixable LSPs are missing (unless show_when_complete)
    if !cfg.show_when_complete && percent >= 100 && !has_fixable_missing {
        return ModuleOutput::hidden();
    }

    let mut vars = HashMap::new();
    vars.insert("percent".into(), percent.to_string());
    vars.insert("total_files".into(), total_files.to_string());
    vars.insert("dirty_files".into(), dirty_files.to_string());
    vars.insert("chunks".into(), chunks.to_string());
    vars.insert("symbols".into(), symbols.to_string());

    let mut parts = Vec::new();

    // Show indexing progress if not complete
    if percent < 100 {
        parts.push(interpolate(&cfg.format, &vars));
    }

    // Show fixable missing LSP prompt
    if has_fixable_missing {
        parts.push("\x1b[33m/lsp to fix\x1b[0m".to_string());
    }

    if parts.is_empty() {
        if cfg.show_when_complete {
            let text = interpolate(&cfg.format, &vars);
            return ModuleOutput::new(text, Style::parse(&cfg.style));
        }
        return ModuleOutput::hidden();
    }

    let text = parts.join(" ");
    ModuleOutput::new(text, Style::parse(&cfg.style))
}

/// Check whether any LSP servers with install hints are missing for present file extensions.
///
/// Returns `true` if at least one LSP server needed by the indexed files is not
/// installed and has a non-empty `install_hint`.
fn has_fixable_missing_lsps(conn: &swissarmyhammer_code_context::DbRef<'_>) -> bool {
    check_fixable_lsps(conn).unwrap_or(false)
}

fn check_fixable_lsps(
    conn: &swissarmyhammer_code_context::DbRef<'_>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let present_exts = swissarmyhammer_code_context::distinct_extensions(conn)?;
    if present_exts.is_empty() {
        return Ok(false);
    }

    let ext_refs: Vec<&str> = present_exts.iter().map(|s| s.as_str()).collect();
    let matching_servers = swissarmyhammer_lsp::servers_for_extensions(&ext_refs);

    Ok(matching_servers.iter().any(|spec| {
        !spec.install_hint.is_empty()
            && swissarmyhammer_code_context::find_executable(&spec.command).is_none()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StatuslineConfig;
    use crate::input::StatuslineInput;

    #[test]
    fn test_index_in_repo() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let _out = eval(&ctx);
    }

    #[test]
    fn test_index_show_when_complete() {
        let input = StatuslineInput::default();
        let mut config = StatuslineConfig::default();
        config.index.show_when_complete = true;
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let _out = eval(&ctx);
    }

    #[test]
    fn test_render_index_incomplete() {
        let cfg = StatuslineConfig::default().index;
        let out = render_index(50, 100, 50, 200, 100, false, &cfg);
        assert!(!out.is_empty());
        assert!(out.text.contains("50"));
    }

    #[test]
    fn test_render_index_complete_hidden() {
        let cfg = StatuslineConfig::default().index;
        let out = render_index(100, 100, 0, 500, 200, false, &cfg);
        assert!(out.is_empty());
    }

    #[test]
    fn test_render_index_complete_shown() {
        let mut cfg = StatuslineConfig::default().index;
        cfg.show_when_complete = true;
        let out = render_index(100, 100, 0, 500, 200, false, &cfg);
        assert!(!out.is_empty());
    }

    #[test]
    fn test_render_index_with_fixable_missing() {
        let cfg = StatuslineConfig::default().index;
        let out = render_index(100, 100, 0, 500, 200, true, &cfg);
        assert!(!out.is_empty());
        assert!(out.text.contains("/lsp to fix"));
    }

    #[test]
    fn test_render_index_incomplete_with_fixable_missing() {
        let cfg = StatuslineConfig::default().index;
        let out = render_index(50, 100, 50, 200, 100, true, &cfg);
        assert!(!out.is_empty());
        assert!(out.text.contains("50"));
        assert!(out.text.contains("/lsp to fix"));
    }

    #[test]
    fn test_render_index_complete_with_show_when_complete_and_fixable() {
        let mut cfg = StatuslineConfig::default().index;
        cfg.show_when_complete = true;
        let out = render_index(100, 100, 0, 500, 200, true, &cfg);
        assert!(!out.is_empty());
        assert!(out.text.contains("/lsp to fix"));
    }

    #[test]
    fn test_render_index_zero_percent() {
        let cfg = StatuslineConfig::default().index;
        let out = render_index(0, 100, 100, 0, 0, false, &cfg);
        assert!(!out.is_empty());
        assert!(out.text.contains("0"));
    }

    #[test]
    fn test_render_index_99_percent() {
        let cfg = StatuslineConfig::default().index;
        let out = render_index(99, 500, 5, 1000, 400, false, &cfg);
        assert!(!out.is_empty());
        assert!(out.text.contains("99"));
    }

    #[test]
    fn test_render_index_render_output() {
        let cfg = StatuslineConfig::default().index;
        let out = render_index(50, 100, 50, 200, 100, false, &cfg);
        let rendered = out.render();
        assert!(rendered.contains("50"));
    }

    #[test]
    fn test_render_index_complete_not_shown_not_fixable() {
        let mut cfg = StatuslineConfig::default().index;
        cfg.show_when_complete = false;
        let out = render_index(100, 100, 0, 500, 200, false, &cfg);
        assert!(out.is_empty());
    }

    #[test]
    fn test_eval_produces_output_or_hidden() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let out = eval(&ctx);
        let _ = out.is_empty();
        let _ = out.render();
    }
}
