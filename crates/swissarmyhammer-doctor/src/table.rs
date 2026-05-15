//! Table rendering utilities.
//!
//! Provides terminal-width-aware table creation and formatted output
//! using comfy-table with colored status symbols.

use crate::display::{CheckResult, VerboseCheckResult};
use crate::types::{Check, CheckStatus};
use comfy_table::{presets::UTF8_FULL, Cell, Color, ContentArrangement, Table};

/// Create a table pre-configured for terminal-width-aware output.
///
/// Uses crossterm to detect the actual terminal width, falling back to
/// 120 columns when not connected to a TTY. All columns use dynamic
/// content arrangement to wrap within the available width.
pub fn new_table() -> Table {
    let width = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(120);

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_width(width);
    table
}

/// Truncate a string to `max` characters, appending "..." if truncated.
///
/// Safe for multi-byte (UTF-8) strings.
pub fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

/// Shorten a package name for display.
///
/// Strips the `https://github.com/` prefix to save space in tables.
pub fn short_name(name: &str) -> String {
    name.strip_prefix("https://github.com/")
        .unwrap_or(name)
        .to_string()
}

/// Print checks as a formatted table.
///
/// Uses comfy-table with UTF8_FULL preset for clean formatting.
/// Status symbols are colored: green for Ok, yellow for Warning, red for Error.
///
/// # Arguments
///
/// * `checks` - Slice of Check results to display
/// * `verbose` - If true, shows additional columns (Fix, Category)
///
/// # Example
///
/// ```
/// use swissarmyhammer_doctor::{Check, CheckStatus, print_checks_table};
///
/// let checks = vec![
///     Check {
///         name: "Test".to_string(),
///         status: CheckStatus::Ok,
///         message: "Passed".to_string(),
///         fix: None,
///     },
/// ];
///
/// print_checks_table(&checks, false);
/// ```
pub fn print_checks_table(checks: &[Check], verbose: bool) {
    let mut table = new_table();

    if verbose {
        table.set_header(vec!["Status", "Check", "Result", "Fix", "Category"]);
        for check in checks {
            let result = VerboseCheckResult::from(check);
            let status_cell = create_status_cell(&check.status);
            table.add_row(vec![
                status_cell,
                Cell::new(&result.name),
                Cell::new(&result.message),
                Cell::new(&result.fix),
                Cell::new(&result.category),
            ]);
        }
    } else {
        table.set_header(vec!["Status", "Check", "Result"]);
        for check in checks {
            let result = CheckResult::from(check);
            let status_cell = create_status_cell(&check.status);
            table.add_row(vec![
                status_cell,
                Cell::new(&result.name),
                Cell::new(&result.message),
            ]);
        }
    }

    println!("{table}");
}

/// Create a colored cell for the status symbol.
fn create_status_cell(status: &CheckStatus) -> Cell {
    match status {
        CheckStatus::Ok => Cell::new("\u{2713}").fg(Color::Green), // check
        CheckStatus::Warning => Cell::new("\u{26A0}").fg(Color::Yellow), // warning
        CheckStatus::Error => Cell::new("\u{2717}").fg(Color::Red), // cross
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_table() {
        let table = new_table();
        drop(table);
    }

    #[test]
    fn test_truncate_str_short() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_str_exact() {
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_str_long() {
        assert_eq!(truncate_str("hello world", 8), "hello...");
    }

    #[test]
    fn test_truncate_str_unicode() {
        let s = "\u{3053}\u{3093}\u{306b}\u{3061}\u{306f}\u{4e16}\u{754c}\u{30c6}\u{30b9}\u{30c8}";
        assert_eq!(truncate_str(s, 6), "\u{3053}\u{3093}\u{306b}...");
    }

    #[test]
    fn test_short_name_github() {
        assert_eq!(
            short_name("https://github.com/owner/repo/skill"),
            "owner/repo/skill"
        );
    }

    #[test]
    fn test_short_name_no_prefix() {
        assert_eq!(short_name("my-local-skill"), "my-local-skill");
    }

    #[test]
    fn test_create_status_cell_ok() {
        let cell = create_status_cell(&CheckStatus::Ok);
        let _ = format!("{:?}", cell);
    }

    #[test]
    fn test_create_status_cell_warning() {
        let cell = create_status_cell(&CheckStatus::Warning);
        let _ = format!("{:?}", cell);
    }

    #[test]
    fn test_create_status_cell_error() {
        let cell = create_status_cell(&CheckStatus::Error);
        let _ = format!("{:?}", cell);
    }

    #[test]
    fn test_print_checks_table_standard() {
        let checks = vec![
            Check {
                name: "Test Check".to_string(),
                status: CheckStatus::Ok,
                message: "Passed".to_string(),
                fix: None,
            },
            Check {
                name: "Warning Check".to_string(),
                status: CheckStatus::Warning,
                message: "Potential issue".to_string(),
                fix: Some("Fix it".to_string()),
            },
        ];

        print_checks_table(&checks, false);
    }

    #[test]
    fn test_print_checks_table_verbose() {
        let checks = vec![Check {
            name: "Test Check".to_string(),
            status: CheckStatus::Error,
            message: "Failed".to_string(),
            fix: Some("Run the fix command".to_string()),
        }];

        print_checks_table(&checks, true);
    }
}
