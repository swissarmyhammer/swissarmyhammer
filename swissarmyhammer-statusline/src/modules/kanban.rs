//! Kanban module - shows kanban board progress as a bar chart.

use std::collections::HashMap;

use crate::module::{interpolate, ModuleContext, ModuleOutput};
use crate::style::Style;

/// Evaluate the kanban module.
///
/// Reads the kanban board task files and shows a progress bar
/// representing the ratio of done tasks to total tasks.
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    // Try to find a .kanban directory
    let kanban_ctx = match swissarmyhammer_kanban::KanbanContext::find(".") {
        Ok(ctx) => ctx,
        Err(_) => return ModuleOutput::hidden(),
    };

    // Count tasks by reading task files
    let tasks_dir = kanban_ctx.tasks_dir();
    if !tasks_dir.exists() {
        return ModuleOutput::hidden();
    }

    let mut total = 0u32;
    let mut done = 0u32;

    if let Ok(entries) = std::fs::read_dir(&tasks_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                total += 1;
                // Read frontmatter to check if task is in a "done" column
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if content.contains("position_column: done")
                        || content.contains("position_column: Done")
                    {
                        done += 1;
                    }
                }
            }
        }
    }

    if total == 0 {
        return ModuleOutput::hidden();
    }

    let cfg = &ctx.config.kanban;
    let pct = (done as f64 / total as f64 * 100.0) as u32;
    let width = cfg.bar_width;
    let filled = ((pct as f64 / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;
    let bar = format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(empty));

    let style_str = if pct < cfg.thresholds.low.below {
        &cfg.thresholds.low.style
    } else if pct < cfg.thresholds.medium.below {
        &cfg.thresholds.medium.style
    } else {
        &cfg.thresholds.high.style
    };

    let mut vars = HashMap::new();
    vars.insert("bar".into(), bar);
    vars.insert("done".into(), done.to_string());
    vars.insert("total".into(), total.to_string());
    let text = interpolate(&cfg.format, &vars);
    ModuleOutput::new(text, Style::parse(style_str))
}
