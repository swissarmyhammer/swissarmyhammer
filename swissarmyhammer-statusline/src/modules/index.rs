//! Index module - shows code-context indexing progress.

use std::collections::HashMap;

use crate::module::{interpolate, ModuleContext, ModuleOutput};
use crate::style::Style;

/// Evaluate the index module.
///
/// Shows the code-context indexing progress percentage.
/// Hidden when indexing is complete (unless `show_when_complete` is set).
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
    if !cfg.show_when_complete && percent >= 100 {
        return ModuleOutput::hidden();
    }

    let mut vars = HashMap::new();
    vars.insert("percent".into(), percent.to_string());
    let text = interpolate(&cfg.format, &vars);
    ModuleOutput::new(text, Style::parse(&cfg.style))
}
