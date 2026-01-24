//! Builtin validators embedded in the AVP binary.
//!
//! This module provides default validators that are always available,
//! regardless of user or project configuration.

use crate::validator::ValidatorLoader;

/// Embedded content for the no-secrets validator.
pub const NO_SECRETS_VALIDATOR: &str = include_str!("../../../builtin/validators/no-secrets.md");

/// Embedded content for the safe-commands validator.
pub const SAFE_COMMANDS_VALIDATOR: &str = include_str!("../../../builtin/validators/safe-commands.md");

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
    loader.add_builtin("no-secrets", NO_SECRETS_VALIDATOR);
    loader.add_builtin("safe-commands", SAFE_COMMANDS_VALIDATOR);
}

/// Get the names of all builtin validators.
pub fn builtin_names() -> &'static [&'static str] {
    &["no-secrets", "safe-commands"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_builtins() {
        let mut loader = ValidatorLoader::new();
        load_builtins(&mut loader);

        assert_eq!(loader.len(), 2);
        assert!(loader.get("no-secrets").is_some());
        assert!(loader.get("safe-commands").is_some());
    }

    #[test]
    fn test_no_secrets_validator_parses() {
        let mut loader = ValidatorLoader::new();
        loader.add_builtin("no-secrets", NO_SECRETS_VALIDATOR);

        let validator = loader.get("no-secrets").unwrap();
        assert_eq!(validator.name(), "no-secrets");
        assert!(validator.body.contains("hardcoded secrets"));
    }

    #[test]
    fn test_safe_commands_validator_parses() {
        let mut loader = ValidatorLoader::new();
        loader.add_builtin("safe-commands", SAFE_COMMANDS_VALIDATOR);

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
}
