//! Builtin validators and YAML includes embedded in the AVP binary.
//!
//! This module provides default validators and YAML include files that are
//! always available, regardless of user or project configuration. Files are
//! automatically discovered from the `builtin/` directory at build time.
//!
//! # YAML Includes
//!
//! YAML files from `builtin/` (excluding subdirectories like `validators/`,
//! `prompts/`, etc.) are loaded as includes. These can be referenced in
//! validator frontmatter using `@path/name` syntax:
//!
//! ```yaml
//! match:
//!   files:
//!     - "@file_groups/source_code"
//! ```

use crate::validator::{ValidatorLoader, ValidatorSource};
use std::path::PathBuf;

// Include the generated builtin YAML includes
include!(concat!(env!("OUT_DIR"), "/builtin_includes.rs"));

/// Load all builtin RuleSets into a loader.
///
/// This loads RuleSets from the builtin/validators directory and also loads
/// builtin YAML includes so that `@` references work.
/// Call this method before loading user or project validators to ensure
/// builtins have the lowest precedence.
///
/// # Example
///
/// ```rust
/// use avp_common::builtin::load_builtins;
/// use avp_common::validator::ValidatorLoader;
///
/// let mut loader = ValidatorLoader::new();
/// load_builtins(&mut loader);
///
/// // Now load user/project validators which will override builtins
/// loader.load_all().ok();
/// ```
pub fn load_builtins(loader: &mut ValidatorLoader) {
    // First load YAML includes so @references work
    for (name, content) in get_builtin_includes() {
        if let Err(e) = loader.add_builtin_include(name, content) {
            tracing::warn!("Failed to load builtin include '{}': {}", name, e);
        }
    }

    // Load RuleSets from builtin/validators directory
    // The path is relative to the crate root where Cargo.toml is located
    let builtin_validators_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../builtin/validators");

    if let Err(e) =
        loader.load_rulesets_directory(&builtin_validators_path, ValidatorSource::Builtin)
    {
        tracing::error!("Failed to load builtin RuleSets: {}", e);
    }
}

/// Get all builtin YAML includes as (name, content) tuples.
pub fn includes_raw() -> Vec<(&'static str, &'static str)> {
    get_builtin_includes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_builtins() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        // Should have loaded at least 5 RuleSets
        assert!(
            loader.ruleset_count() >= 5,
            "Should have loaded at least 5 RuleSets"
        );

        // Check for expected RuleSets
        assert!(
            loader.get_ruleset("security-rules").is_some(),
            "Should have security-rules RuleSet"
        );
        assert!(
            loader.get_ruleset("command-safety").is_some(),
            "Should have command-safety RuleSet"
        );
        assert!(
            loader.get_ruleset("code-quality").is_some(),
            "Should have code-quality RuleSet"
        );
    }

    #[test]
    fn test_security_rules_ruleset_loads() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let ruleset = loader
            .get_ruleset("security-rules")
            .expect("security-rules RuleSet should exist");
        assert_eq!(ruleset.name(), "security-rules");
        assert!(
            ruleset.rules.len() >= 2,
            "Should have at least 2 rules (no-secrets, input-validation)"
        );

        // Check for no-secrets rule
        let has_no_secrets = ruleset.rules.iter().any(|r| r.name == "no-secrets");
        assert!(has_no_secrets, "Should have no-secrets rule");
    }

    #[test]
    fn test_command_safety_ruleset_loads() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let ruleset = loader
            .get_ruleset("command-safety")
            .expect("command-safety RuleSet should exist");
        assert_eq!(ruleset.name(), "command-safety");

        // Check for safe-commands rule
        let has_safe_commands = ruleset.rules.iter().any(|r| r.name == "safe-commands");
        assert!(has_safe_commands, "Should have safe-commands rule");
    }

    #[test]
    fn test_code_quality_ruleset_loads() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let ruleset = loader
            .get_ruleset("code-quality")
            .expect("code-quality RuleSet should exist");
        assert_eq!(ruleset.name(), "code-quality");
        assert!(
            ruleset.rules.len() >= 10,
            "Should have at least 10 code quality rules"
        );
    }

    #[test]
    fn test_builtin_includes_loaded() {
        let includes = get_builtin_includes();
        assert!(
            !includes.is_empty(),
            "Should have at least one builtin include"
        );

        // Should have file_groups
        let names: Vec<&str> = includes.iter().map(|(name, _)| *name).collect();
        assert!(
            names.iter().any(|n| n.contains("file_groups")),
            "Should have file_groups includes"
        );
    }

    #[test]
    fn test_security_rules_expands_file_groups() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let ruleset = loader
            .get_ruleset("security-rules")
            .expect("security-rules should be loaded");

        // The @file_groups/source_code should have been expanded in the manifest
        let match_criteria = ruleset
            .manifest
            .match_criteria
            .as_ref()
            .expect("security-rules should have match criteria");

        // Should have actual file patterns, not the @reference
        assert!(
            !match_criteria.files.is_empty(),
            "files should not be empty after expansion"
        );
        assert!(
            !match_criteria.files.iter().any(|f| f.starts_with('@')),
            "@ references should be expanded, but found: {:?}",
            match_criteria.files
        );
        // Should contain some expected patterns from source_code.yaml
        assert!(
            match_criteria
                .files
                .iter()
                .any(|f| f == "*.js" || f == "*.ts" || f == "*.py"),
            "Should contain common source file patterns after expansion"
        );
    }

    #[test]
    fn test_test_integrity_expands_file_groups() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let ruleset = loader
            .get_ruleset("test-integrity")
            .expect("test-integrity should be loaded");

        // The @file_groups/source_code and @file_groups/test_files should have been expanded
        let match_criteria = ruleset
            .manifest
            .match_criteria
            .as_ref()
            .expect("test-integrity should have match criteria");

        // Should have actual file patterns, not the @reference
        assert!(
            !match_criteria.files.is_empty(),
            "files should not be empty after expansion"
        );
        assert!(
            !match_criteria.files.iter().any(|f| f.starts_with('@')),
            "@ references should be expanded, but found: {:?}",
            match_criteria.files
        );
        // Should contain patterns from both source_code.yaml and test_files.yaml
        assert!(
            match_criteria
                .files
                .iter()
                .any(|f| f == "*.js" || f == "*.ts" || f == "*.py"),
            "Should contain source file patterns after expansion"
        );
        assert!(
            match_criteria
                .files
                .iter()
                .any(|f| f.contains("test") || f.contains("spec")),
            "Should contain test file patterns after expansion"
        );
    }
}
