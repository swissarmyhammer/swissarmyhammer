//! Languages module - shows language icons based on code-context indexed files.
//!
//! Derives language presence from the actual file extensions tracked in the
//! code-context database via `distinct_extensions()`, and maps extensions to
//! icons using the LSP registry rather than hardcoded tables.

use std::collections::BTreeSet;

use crate::module::{ModuleContext, ModuleOutput};
use crate::style::Style;

/// Default icon used when an LSP spec has no icon configured.
const DEFAULT_ICON: &str = "\u{1f4e6}";

/// Evaluate the languages module.
///
/// Queries the code-context database for actual file extensions, then looks up
/// matching LSP server specs from the registry. Shows the spec's icon for each
/// detected language. Icons are dimmed when the corresponding LSP server is not
/// found in PATH. Deduplicates icons so that multiple servers for the same
/// language (e.g. pyright and pylsp) only show one icon.
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
    let cfg = &ctx.config.languages;

    let present_exts = match swissarmyhammer_code_context::distinct_extensions(&conn) {
        Ok(exts) => exts,
        Err(_) => return ModuleOutput::hidden(),
    };

    if present_exts.is_empty() {
        return ModuleOutput::hidden();
    }

    let ext_refs: Vec<&str> = present_exts.iter().map(|s| s.as_str()).collect();
    let matching_servers = swissarmyhammer_lsp::servers_for_extensions(&ext_refs);

    // Collect (icon, has_lsp) pairs, deduplicating by icon text.
    // Use BTreeSet for deterministic ordering.
    let mut seen_icons = BTreeSet::new();
    let mut icons = Vec::new();

    for spec in &matching_servers {
        let icon = spec.icon.as_deref().unwrap_or(DEFAULT_ICON);

        if !seen_icons.insert(icon.to_string()) {
            continue;
        }

        let has_lsp =
            swissarmyhammer_code_context::find_executable(&spec.command).is_some();

        if has_lsp || !cfg.dim_without_lsp {
            icons.push(icon.to_string());
        } else {
            icons.push(format!("\x1b[2m{}\x1b[22m", icon));
        }
    }

    if icons.is_empty() {
        return ModuleOutput::hidden();
    }

    let text = icons.join(" ");
    ModuleOutput::new(text, Style::parse(&cfg.style))
}
