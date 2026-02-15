//! Package type detection and shared name validation.
//!
//! Mirdan manages two package types:
//! - **Skill**: Contains SKILL.md (agentskills.io spec)
//! - **Validator**: Contains VALIDATOR.md + rules/ directory (AVP spec)

use std::fmt;
use std::path::Path;

/// The two package types Mirdan manages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageType {
    Skill,
    Validator,
}

impl fmt::Display for PackageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageType::Skill => write!(f, "skill"),
            PackageType::Validator => write!(f, "validator"),
        }
    }
}

/// Detect package type from a directory's contents.
///
/// - SKILL.md present -> Skill
/// - VALIDATOR.md present AND rules/ directory exists -> Validator
/// - Otherwise -> None
pub fn detect_package_type(dir: &Path) -> Option<PackageType> {
    if dir.join("SKILL.md").exists() {
        Some(PackageType::Skill)
    } else if dir.join("VALIDATOR.md").exists() && dir.join("rules").is_dir() {
        Some(PackageType::Validator)
    } else {
        None
    }
}

/// Validate that a package name is valid.
///
/// Rules (shared by agentskills.io and AVP):
/// - 1-64 characters
/// - Lowercase alphanumeric and hyphens only
/// - No leading, trailing, or consecutive hyphens
pub fn is_valid_package_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }
    if name.starts_with('-') || name.ends_with('-') {
        return false;
    }
    if name.contains("--") {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_package_names() {
        assert!(is_valid_package_name("no-secrets"));
        assert!(is_valid_package_name("a"));
        assert!(is_valid_package_name("my-validator-123"));
        assert!(is_valid_package_name("abc"));
        assert!(is_valid_package_name("a-b-c"));
    }

    #[test]
    fn test_invalid_package_names() {
        assert!(!is_valid_package_name(""));
        assert!(!is_valid_package_name("-starts-with-hyphen"));
        assert!(!is_valid_package_name("ends-with-hyphen-"));
        assert!(!is_valid_package_name("HAS_UPPER"));
        assert!(!is_valid_package_name("has spaces"));
        assert!(!is_valid_package_name("has_underscores"));
        assert!(!is_valid_package_name("double--hyphen"));
        let long = "a".repeat(65);
        assert!(!is_valid_package_name(&long));
    }

    #[test]
    fn test_package_type_display() {
        assert_eq!(PackageType::Skill.to_string(), "skill");
        assert_eq!(PackageType::Validator.to_string(), "validator");
    }

    #[test]
    fn test_detect_package_type_skill() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("SKILL.md"), "# Skill").unwrap();
        assert_eq!(detect_package_type(dir.path()), Some(PackageType::Skill));
    }

    #[test]
    fn test_detect_package_type_validator() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("VALIDATOR.md"), "---\nname: test\n---").unwrap();
        std::fs::create_dir(dir.path().join("rules")).unwrap();
        assert_eq!(
            detect_package_type(dir.path()),
            Some(PackageType::Validator)
        );
    }

    #[test]
    fn test_detect_package_type_validator_without_rules() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("VALIDATOR.md"), "---\nname: test\n---").unwrap();
        // No rules/ directory -- should not detect as validator
        assert_eq!(detect_package_type(dir.path()), None);
    }

    #[test]
    fn test_detect_package_type_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(detect_package_type(dir.path()), None);
    }

    #[test]
    fn test_package_type_serde() {
        let json = serde_json::to_string(&PackageType::Skill).unwrap();
        assert_eq!(json, "\"skill\"");
        let parsed: PackageType = serde_json::from_str("\"validator\"").unwrap();
        assert_eq!(parsed, PackageType::Validator);
    }
}
