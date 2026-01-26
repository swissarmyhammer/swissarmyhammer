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

use crate::validator::ValidatorLoader;

// Include the generated builtin validators
include!(concat!(env!("OUT_DIR"), "/builtin_validators.rs"));

// Include the generated builtin YAML includes
include!(concat!(env!("OUT_DIR"), "/builtin_includes.rs"));

/// Load all builtin validators into a loader.
///
/// This also loads builtin YAML includes so that `@` references work.
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

    // Then load validators
    for (name, content) in get_builtin_validators() {
        loader.add_builtin(name, content);
    }
}

/// Get all builtin YAML includes as (name, content) tuples.
pub fn includes_raw() -> Vec<(&'static str, &'static str)> {
    get_builtin_includes()
}

/// Get the names of all builtin validators.
pub fn builtin_names() -> Vec<&'static str> {
    get_builtin_validators()
        .into_iter()
        .map(|(name, _)| name)
        .collect()
}

/// Get all builtin validators as (name, content) tuples.
///
/// This provides direct access to the raw builtin validator content,
/// which is useful for extracting partials for templating.
pub fn validators_raw() -> Vec<(&'static str, &'static str)> {
    get_builtin_validators()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_builtins() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        // Should have loaded at least 2 validators (no-secrets, safe-commands)
        assert!(loader.len() >= 2);
        assert!(loader.get("no-secrets").is_some());
        assert!(loader.get("safe-commands").is_some());
    }

    #[test]
    fn test_no_secrets_validator_parses() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let validator = loader.get("no-secrets").unwrap();
        assert_eq!(validator.name(), "no-secrets");
        assert!(validator.body.contains("hardcoded secrets"));
    }

    #[test]
    fn test_safe_commands_validator_parses() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let validator = loader.get("safe-commands").unwrap();
        assert_eq!(validator.name(), "safe-commands");
        assert!(validator.body.contains("dangerous"));
    }

    #[test]
    fn test_builtin_names() {
        let names = builtin_names();
        assert!(names.contains(&"no-secrets"));
        assert!(names.contains(&"safe-commands"));
    }

    #[test]
    fn test_get_builtin_validators() {
        let validators = get_builtin_validators();
        assert!(
            !validators.is_empty(),
            "Should have at least one builtin validator"
        );

        // Each validator should have a non-empty name and content
        for (name, content) in validators {
            assert!(!name.is_empty(), "Validator name should not be empty");
            assert!(!content.is_empty(), "Validator content should not be empty");
        }
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
    fn test_no_secrets_expands_file_groups() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let validator = loader.get("no-secrets").unwrap();
        
        // The @file_groups/source_code should have been expanded
        let match_criteria = validator.frontmatter.match_criteria.as_ref()
            .expect("no-secrets should have match criteria");
        
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
            match_criteria.files.iter().any(|f| f == "*.js" || f == "*.ts" || f == "*.py"),
            "Should contain common source file patterns after expansion"
        );
    }

    #[test]
    fn test_no_test_cheating_expands_file_groups() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        let validator = loader.get("no-test-cheating").unwrap();
        
        // The @file_groups/source_code and @file_groups/test_files should have been expanded
        let match_criteria = validator.frontmatter.match_criteria.as_ref()
            .expect("no-test-cheating should have match criteria");
        
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
            match_criteria.files.iter().any(|f| f == "*.js" || f == "*.ts" || f == "*.py"),
            "Should contain source file patterns after expansion"
        );
        assert!(
            match_criteria.files.iter().any(|f| f.contains("test") || f.contains("spec")),
            "Should contain test file patterns after expansion"
        );
    }
}
