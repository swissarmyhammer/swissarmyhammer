//! AVP List - List all available validators.
//!
//! Lists validators from all sources (builtin, user, project) with their
//! name, description, trigger, severity, and source.

use avp_common::builtin::load_builtins;
use avp_common::validator::{ValidatorLoader, ValidatorSource};
use comfy_table::{presets::UTF8_FULL, Table};

/// Maximum length for description in table display before truncation.
const MAX_DESCRIPTION_LENGTH: usize = 50;

/// Run the list command and display validators.
///
/// Loads validators from all sources (builtin, user, project) and displays
/// them in a formatted table. User and project validators override builtins
/// with the same name.
///
/// # Arguments
///
/// * `verbose` - If true, includes description column in output.
/// * `debug` - If true, shows diagnostic information about directories searched.
///
/// # Returns
///
/// Exit code: 0 on success.
pub fn run_list(verbose: bool, debug: bool) -> i32 {
    let mut loader = ValidatorLoader::new();

    // Load builtins first (lowest precedence)
    load_builtins(&mut loader);

    // Load user and project validators (will override builtins with same name)
    if let Err(e) = loader.load_all() {
        eprintln!("Warning: Failed to load some validators: {}", e);
    }

    // Show diagnostics if debug mode is enabled
    if debug {
        print_diagnostics(&loader);
    }

    let mut validators = loader.list();

    if validators.is_empty() {
        println!("No validators found.");
        return 0;
    }

    // Sort by name for consistent output
    validators.sort_by(|a, b| a.name().cmp(b.name()));

    // Build and print the table
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);

    // Set headers based on verbose mode
    if verbose {
        table.set_header(vec!["Name", "Description", "Trigger", "Severity", "Source"]);
    } else {
        table.set_header(vec!["Name", "Trigger", "Severity", "Source"]);
    }

    // Add rows
    for v in &validators {
        let source = source_emoji(&v.source);
        if verbose {
            let description = truncate_description(v.description(), MAX_DESCRIPTION_LENGTH);
            table.add_row(vec![
                v.name(),
                &description,
                &v.trigger().to_string(),
                &v.severity().to_string(),
                &source,
            ]);
        } else {
            table.add_row(vec![
                v.name(),
                &v.trigger().to_string(),
                &v.severity().to_string(),
                &source,
            ]);
        }
    }

    println!("{table}");
    println!();
    println!("{} validator(s) found", validators.len());

    0
}

/// Get emoji representation for validator source.
///
/// Maps each source type to a user-friendly string with emoji prefix.
fn source_emoji(source: &ValidatorSource) -> String {
    match source {
        ValidatorSource::Builtin => "📦 Built-in".to_string(),
        ValidatorSource::User => "👤 User".to_string(),
        ValidatorSource::Project => "📁 Project".to_string(),
    }
}

/// Truncate description to max length with ellipsis.
///
/// If the description exceeds max_len, truncates and appends "...".
/// This is for user-facing table display formatting only.
fn truncate_description(desc: &str, max_len: usize) -> String {
    if desc.len() <= max_len {
        desc.to_string()
    } else {
        format!("{}...", &desc[..max_len - 3])
    }
}

/// Print diagnostic information about validator loading.
///
/// Shows which directories are being searched and counts by source.
fn print_diagnostics(loader: &ValidatorLoader) {
    let diag = loader.diagnostics();

    println!("=== Validator Loading Diagnostics ===");
    println!();

    // User directory info
    println!("User directory (~/.avp/validators):");
    match &diag.user_directory.path {
        Some(path) => {
            println!("  Path: {}", path.display());
            if diag.user_directory.exists {
                println!("  Status: ✓ exists");
            } else {
                println!("  Status: ✗ does not exist (create this directory to add user validators)");
            }
        }
        None => {
            if let Some(err) = &diag.user_directory.error {
                println!("  Status: ✗ could not resolve ({})", err);
            } else {
                println!("  Status: ✗ could not resolve home directory");
            }
        }
    }
    println!();

    // Project directory info
    println!("Project directory (.avp/validators):");
    match &diag.project_directory.path {
        Some(path) => {
            println!("  Path: {}", path.display());
            if diag.project_directory.exists {
                println!("  Status: ✓ exists");
            } else {
                println!("  Status: ✗ does not exist");
            }
        }
        None => {
            if let Some(err) = &diag.project_directory.error {
                println!("  Status: ✗ could not resolve ({})", err);
            } else {
                println!("  Status: ✗ not in a git repository");
            }
        }
    }
    println!();

    // Counts
    println!("Validators loaded:");
    println!("  📦 Built-in: {}", diag.builtin_count);
    println!("  👤 User: {}", diag.user_count);
    println!("  📁 Project: {}", diag.project_count);
    println!("  Total: {}", diag.total_count);
    println!();
    println!("=========================================");
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_emoji() {
        assert_eq!(source_emoji(&ValidatorSource::Builtin), "📦 Built-in");
        assert_eq!(source_emoji(&ValidatorSource::User), "👤 User");
        assert_eq!(source_emoji(&ValidatorSource::Project), "📁 Project");
    }

    /// Test length for truncation tests.
    const TEST_TRUNCATE_LEN: usize = 20;

    #[test]
    fn test_truncate_description_short() {
        let desc = "Short description";
        assert_eq!(
            truncate_description(desc, MAX_DESCRIPTION_LENGTH),
            "Short description"
        );
    }

    #[test]
    fn test_truncate_description_long() {
        let desc = "This is a very long description that should be truncated";
        let result = truncate_description(desc, TEST_TRUNCATE_LEN);
        assert_eq!(result.len(), TEST_TRUNCATE_LEN);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_description_exact() {
        let desc = "Exactly twenty chars";
        assert_eq!(truncate_description(desc, TEST_TRUNCATE_LEN), desc);
    }

    #[test]
    fn test_run_list() {
        // Should not panic and return 0
        let exit_code = run_list(false, false);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_list_verbose() {
        // Should not panic and return 0
        let exit_code = run_list(true, false);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_list_debug() {
        // Should not panic and return 0, and print diagnostics
        let exit_code = run_list(false, true);
        assert_eq!(exit_code, 0);
    }
}
