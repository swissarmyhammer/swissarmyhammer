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

    let (done, total) = count_tasks_in_dir(&tasks_dir);
    render_kanban_bar(done, total, &ctx.config.kanban)
}

/// Count tasks in a directory by reading `.md` files and checking frontmatter.
fn count_tasks_in_dir(tasks_dir: &std::path::Path) -> (u32, u32) {
    let mut total = 0u32;
    let mut done = 0u32;

    if let Ok(entries) = std::fs::read_dir(tasks_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                total += 1;
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

    (done, total)
}

/// Render a kanban progress bar from task counts.
fn render_kanban_bar(
    done: u32,
    total: u32,
    cfg: &crate::config::KanbanModuleConfig,
) -> ModuleOutput {
    if total == 0 {
        return ModuleOutput::hidden();
    }

    let pct = (done as f64 / total as f64 * 100.0) as u32;
    let width = cfg.bar_width;
    let filled = ((pct as f64 / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;
    let bar = format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(empty));

    let mut vars = HashMap::new();
    vars.insert("bar".into(), bar);
    vars.insert("done".into(), done.to_string());
    vars.insert("total".into(), total.to_string());
    let text = interpolate(&cfg.format, &vars);
    ModuleOutput::new(text, Style::parse(&cfg.style))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StatuslineConfig;
    use crate::input::StatuslineInput;

    #[test]
    fn test_kanban_in_repo() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let _out = eval(&ctx);
    }

    #[test]
    fn test_render_kanban_bar_zero_total() {
        let cfg = StatuslineConfig::default().kanban;
        let out = render_kanban_bar(0, 0, &cfg);
        assert!(out.is_empty());
    }

    #[test]
    fn test_render_kanban_bar_partial() {
        let cfg = StatuslineConfig::default().kanban;
        let out = render_kanban_bar(3, 10, &cfg);
        assert!(!out.is_empty());
        assert!(out.text.contains("3"));
        assert!(out.text.contains("10"));
    }

    #[test]
    fn test_render_kanban_bar_all_done() {
        let cfg = StatuslineConfig::default().kanban;
        let out = render_kanban_bar(10, 10, &cfg);
        assert!(!out.is_empty());
        assert!(out.text.contains("10/10"));
    }

    #[test]
    fn test_render_kanban_bar_none_done() {
        let cfg = StatuslineConfig::default().kanban;
        let out = render_kanban_bar(0, 5, &cfg);
        assert!(!out.is_empty());
        assert!(out.text.contains("0"));
        assert!(out.text.contains("5"));
    }

    #[test]
    fn test_count_tasks_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let (done, total) = count_tasks_in_dir(dir.path());
        assert_eq!(done, 0);
        assert_eq!(total, 0);
    }

    #[test]
    fn test_count_tasks_with_tasks() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("task1.md"),
            "---\nposition_column: todo\n---\nTask 1\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("task2.md"),
            "---\nposition_column: done\n---\nTask 2\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("task3.md"),
            "---\nposition_column: Done\n---\nTask 3\n",
        )
        .unwrap();
        // Non-md files should be ignored
        std::fs::write(dir.path().join("readme.txt"), "not a task").unwrap();

        let (done, total) = count_tasks_in_dir(dir.path());
        assert_eq!(total, 3);
        assert_eq!(done, 2);
    }

    #[test]
    fn test_count_tasks_nonexistent_dir() {
        let (done, total) = count_tasks_in_dir(std::path::Path::new("/nonexistent/path"));
        assert_eq!(done, 0);
        assert_eq!(total, 0);
    }
}
