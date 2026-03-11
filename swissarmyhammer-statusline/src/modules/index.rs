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
    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(_) => return ModuleOutput::hidden(),
    };

    let ws = match swissarmyhammer_code_context::CodeContextWorkspace::open(&cwd) {
        Ok(ws) => ws,
        Err(_) => return ModuleOutput::hidden(),
    };

    let conn = ws.db();
    let status = match swissarmyhammer_code_context::get_status(&conn) {
        Ok(s) => s,
        Err(_) => return ModuleOutput::hidden(),
    };

    let cfg = &ctx.config.index;
    let percent = status.ts_indexed_percent as u32;

    // Determine which LSP servers are missing and fixable.
    let has_fixable_missing = has_fixable_missing_lsps(&conn);

    // Hide when indexing is complete AND no fixable LSPs are missing (unless show_when_complete)
    if !cfg.show_when_complete && percent >= 100 && !has_fixable_missing {
        return ModuleOutput::hidden();
    }

    let mut vars = HashMap::new();
    vars.insert("percent".into(), percent.to_string());
    vars.insert("total_files".into(), status.total_files.to_string());
    vars.insert("dirty_files".into(), status.dirty_files.to_string());
    vars.insert("chunks".into(), status.ts_chunk_count.to_string());
    vars.insert("symbols".into(), status.lsp_symbol_count.to_string());

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
    let present_exts = match swissarmyhammer_code_context::distinct_extensions(conn) {
        Ok(exts) => exts,
        Err(_) => return false,
    };

    if present_exts.is_empty() {
        return false;
    }

    let ext_refs: Vec<&str> = present_exts.iter().map(|s| s.as_str()).collect();
    let matching_servers = swissarmyhammer_lsp::servers_for_extensions(&ext_refs);

    matching_servers.iter().any(|spec| {
        !spec.install_hint.is_empty()
            && swissarmyhammer_code_context::find_executable(&spec.command).is_none()
    })
}
