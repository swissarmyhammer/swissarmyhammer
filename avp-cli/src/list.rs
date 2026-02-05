//! AVP List - List all available validators.
//!
//! Lists RuleSets and their rules from all sources (builtin, user, project)
//! with name, description, trigger, severity, and source.

use avp_common::builtin::load_builtins;
use avp_common::validator::{ValidatorLoader, ValidatorSource};
use comfy_table::{presets::UTF8_FULL, Table};

/// Maximum length for description in table display before truncation.
const MAX_DESCRIPTION_LENGTH: usize = 50;

/// Run the list command and display RuleSets and their rules.
///
/// Loads RuleSets from all sources (builtin, user, project) and displays
/// them in a formatted table. User and project RuleSets override builtins
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

    let mut rulesets = loader.list_rulesets();

    if rulesets.is_empty() {
        println!("No RuleSets found.");
        return 0;
    }

    // Sort by name for consistent output
    rulesets.sort_by(|a, b| a.name().cmp(b.name()));

    // Build and print the table - one row per RuleSet, rules as multiline cell
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);

    if verbose {
        table.set_header(vec!["RuleSet", "Description", "Rules", "Trigger", "Severity", "Source"]);
    } else {
        table.set_header(vec!["RuleSet", "Rules", "Trigger", "Severity", "Source"]);
    }

    for ruleset in &rulesets {
        let source = source_emoji(&ruleset.source);
        let trigger = ruleset.manifest.trigger.to_string();
        let severity = ruleset.manifest.severity.to_string();

        // Build multiline rules cell
        let rules_cell: String = ruleset
            .rules
            .iter()
            .map(|rule| {
                let eff_sev = rule.effective_severity(ruleset);
                if eff_sev != ruleset.manifest.severity {
                    format!("{} [{}]", rule.name, eff_sev)
                } else {
                    rule.name.clone()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        if verbose {
            let description = truncate_description(ruleset.description(), MAX_DESCRIPTION_LENGTH);
            table.add_row(vec![
                ruleset.name().to_string(),
                description,
                rules_cell,
                trigger,
                severity,
                source,
            ]);
        } else {
            table.add_row(vec![
                ruleset.name().to_string(),
                rules_cell,
                trigger,
                severity,
                source,
            ]);
        }
    }

    println!("{table}");

    let total_rules: usize = rulesets.iter().map(|rs| rs.rules.len()).sum();
    println!(
        "\n{} RuleSet(s), {} rule(s)",
        rulesets.len(),
        total_rules
    );

    0
}

/// Get emoji representation for validator source.
fn source_emoji(source: &ValidatorSource) -> String {
    match source {
        ValidatorSource::Builtin => "builtin".to_string(),
        ValidatorSource::User => "user".to_string(),
        ValidatorSource::Project => "project".to_string(),
    }
}

/// Truncate description to max length with ellipsis.
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
                println!("  Status: exists");
            } else {
                println!(
                    "  Status: does not exist (create this directory to add user validators)"
                );
            }
        }
        None => {
            if let Some(err) = &diag.user_directory.error {
                println!("  Status: could not resolve ({})", err);
            } else {
                println!("  Status: could not resolve home directory");
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
                println!("  Status: exists");
            } else {
                println!("  Status: does not exist");
            }
        }
        None => {
            if let Some(err) = &diag.project_directory.error {
                println!("  Status: could not resolve ({})", err);
            } else {
                println!("  Status: not in a git repository");
            }
        }
    }
    println!();

    // Counts
    println!("Validators loaded:");
    println!("  Built-in: {}", diag.builtin_count);
    println!("  User: {}", diag.user_count);
    println!("  Project: {}", diag.project_count);
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
        assert_eq!(source_emoji(&ValidatorSource::Builtin), "builtin");
        assert_eq!(source_emoji(&ValidatorSource::User), "user");
        assert_eq!(source_emoji(&ValidatorSource::Project), "project");
    }

    #[test]
    fn test_truncate_description_short() {
        assert_eq!(
            truncate_description("Short", MAX_DESCRIPTION_LENGTH),
            "Short"
        );
    }

    #[test]
    fn test_truncate_description_long() {
        let long = "This is a very long description that definitely exceeds the maximum length";
        let result = truncate_description(long, MAX_DESCRIPTION_LENGTH);
        assert_eq!(result.len(), MAX_DESCRIPTION_LENGTH);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_run_list() {
        let exit_code = run_list(false, false);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_list_verbose() {
        let exit_code = run_list(true, false);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_list_debug() {
        let exit_code = run_list(false, true);
        assert_eq!(exit_code, 0);
    }
}
