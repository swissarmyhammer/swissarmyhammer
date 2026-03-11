//! Index module - shows code-context indexing progress and LSP health.

use std::collections::HashMap;

use crate::module::{interpolate, ModuleContext, ModuleOutput};
use crate::style::Style;

/// File extensions and their corresponding LSP server executables.
const LSP_SERVERS: &[(&[&str], &str)] = &[
    (&["rs"], "rust-analyzer"),
    (&["py"], "pyright"),
    (&["ts", "tsx", "js", "jsx"], "typescript-language-server"),
    (&["go"], "gopls"),
    (&["java"], "jdtls"),
    (&["cs"], "omnisharp"),
    (&["c", "cpp", "cc", "cxx", "h", "hpp", "hxx"], "clangd"),
    (&["dart"], "dart"),
    (&["php"], "intelephense"),
    (&["rb"], "solargraph"),
    (&["swift"], "sourcekit-lsp"),
    (&["kt", "kts"], "kotlin-language-server"),
];

/// Check if the database has files with the given extension.
fn has_extension(conn: &rusqlite::Connection, ext: &str) -> bool {
    let pattern = format!("%.{}", ext);
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM indexed_files WHERE file_path LIKE ?1 LIMIT 1)",
        [&pattern],
        |row| row.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

/// Find LSP servers that are missing for languages present in the index.
fn find_missing_lsps(conn: &rusqlite::Connection) -> Vec<&'static str> {
    let mut missing = Vec::new();
    for (extensions, server) in LSP_SERVERS {
        let has_files = extensions.iter().any(|ext| has_extension(conn, ext));
        if has_files && swissarmyhammer_code_context::find_executable(server).is_none() {
            missing.push(*server);
        }
    }
    missing
}

/// Evaluate the index module.
///
/// Shows the code-context indexing progress percentage and missing LSP warnings.
/// Hidden when indexing is complete and no LSPs are missing (unless `show_when_complete` is set).
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    // Try to open code-context workspace as reader
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
    let missing_lsps = find_missing_lsps(&conn);

    // Hide when indexing is complete AND no LSPs are missing (unless show_when_complete)
    if !cfg.show_when_complete && percent >= 100 && missing_lsps.is_empty() {
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

    // Show missing LSP warning
    if !missing_lsps.is_empty() {
        let lsp_list = missing_lsps.join(",");
        parts.push(format!("\x1b[33mmissing: {}\x1b[0m", lsp_list));
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
