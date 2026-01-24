//! Builtin validators embedded in the AVP binary.
//!
//! This module provides default validators that are always available,
//! regardless of user or project configuration. Validators are automatically
//! discovered from the `builtin/validators` directory at build time.

use crate::validator::ValidatorLoader;

// Include the generated builtin validators
include!(concat!(env!("OUT_DIR"), "/builtin_validators.rs"));

/// Load all builtin validators into a loader.
///
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
    for (name, content) in get_builtin_validators() {
        loader.add_builtin(name, content);
    }
}

/// Get the names of all builtin validators.
pub fn builtin_names() -> Vec<&'static str> {
    get_builtin_validators().into_iter().map(|(name, _)| name).collect()
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
        assert!(!validators.is_empty(), "Should have at least one builtin validator");

        // Each validator should have a non-empty name and content
        for (name, content) in validators {
            assert!(!name.is_empty(), "Validator name should not be empty");
            assert!(!content.is_empty(), "Validator content should not be empty");
        }
    }
}
