//! Table rendering for doctor command output
//!
//! Provides formatted table output using comfy-table with colored status symbols.

use crate::display::{CheckResult, VerboseCheckResult};
use crate::types::{Check, CheckStatus};
use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};

/// Print checks as a formatted table
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
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);

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

/// Create a colored cell for the status symbol
fn create_status_cell(status: &CheckStatus) -> Cell {
    match status {
        CheckStatus::Ok => Cell::new("\u{2713}").fg(Color::Green), // ✓
        CheckStatus::Warning => Cell::new("\u{26A0}").fg(Color::Yellow), // ⚠
        CheckStatus::Error => Cell::new("\u{2717}").fg(Color::Red), // ✗
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_status_cell_ok() {
        let cell = create_status_cell(&CheckStatus::Ok);
        // Cell is created successfully - we can't easily test the color
        // but we can verify it doesn't panic
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
        // Just verify it doesn't panic
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

        // Note: This will print to stdout during tests
        // In a real test environment, you might want to capture stdout
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
