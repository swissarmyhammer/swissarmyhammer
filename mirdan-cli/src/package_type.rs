//! Package type detection and shared name validation.
//!
//! Mirdan manages four package types:
//! - **Skill**: Contains SKILL.md (agentskills.io spec)
//! - **Validator**: Contains VALIDATOR.md + rules/ directory (AVP spec)
//! - **Tool**: Contains TOOL.md (MCP server definition)
//! - **Plugin**: Contains .claude-plugin/plugin.json (Claude Code plugin)

use std::fmt;
use std::path::Path;

/// The four package types Mirdan manages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageType {
    Skill,
    Validator,
    Tool,
    Plugin,
}

impl fmt::Display for PackageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageType::Skill => write!(f, "skill"),
            PackageType::Validator => write!(f, "validator"),
            PackageType::Tool => write!(f, "tool"),
            PackageType::Plugin => write!(f, "plugin"),
        }
    }
}

/// Detect package type from a directory's contents.
///
/// Detection order:
/// 1. SKILL.md present -> Skill
/// 2. VALIDATOR.md present AND rules/ directory exists -> Validator
/// 3. TOOL.md present -> Tool
/// 4. .claude-plugin/plugin.json present -> Plugin
/// 5. Otherwise -> None
pub fn detect_package_type(dir: &Path) -> Option<PackageType> {
    if dir.join("SKILL.md").exists() {
        Some(PackageType::Skill)
    } else if dir.join("VALIDATOR.md").exists() && dir.join("rules").is_dir() {
        Some(PackageType::Validator)
    } else if dir.join("TOOL.md").exists() {
        Some(PackageType::Tool)
    } else if dir.join(".claude-plugin").join("plugin.json").exists() {
        Some(PackageType::Plugin)
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
        assert_eq!(PackageType::Tool.to_string(), "tool");
        assert_eq!(PackageType::Plugin.to_string(), "plugin");
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
    fn test_detect_package_type_tool() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("TOOL.md"), "---\nname: test\n---").unwrap();
        assert_eq!(detect_package_type(dir.path()), Some(PackageType::Tool));
    }

    #[test]
    fn test_detect_package_type_plugin() {
        let dir = tempfile::tempdir().unwrap();
        let plugin_dir = dir.path().join(".claude-plugin");
        std::fs::create_dir(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("plugin.json"),
            r#"{"name": "test", "description": "test"}"#,
        )
        .unwrap();
        assert_eq!(detect_package_type(dir.path()), Some(PackageType::Plugin));
    }

    #[test]
    fn test_detect_package_type_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(detect_package_type(dir.path()), None);
    }

    #[test]
    fn test_invalid_names_path_traversal() {
        assert!(!is_valid_package_name("../etc"));
        assert!(!is_valid_package_name("foo/../../bar"));
        assert!(!is_valid_package_name(".."));
        assert!(!is_valid_package_name("foo/bar"));
        assert!(!is_valid_package_name("a/b"));
    }

    #[test]
    fn test_invalid_names_special_chars() {
        assert!(!is_valid_package_name("foo\0bar"));
        assert!(!is_valid_package_name("foo\\bar"));
        assert!(!is_valid_package_name("."));
        assert!(!is_valid_package_name("..."));
    }

    #[test]
    fn test_valid_name_boundary() {
        // Exactly 64 chars (max valid)
        let max_name = "a".repeat(64);
        assert!(is_valid_package_name(&max_name));
        // Single char
        assert!(is_valid_package_name("a"));
    }

    #[test]
    fn test_package_type_serde() {
        let json = serde_json::to_string(&PackageType::Skill).unwrap();
        assert_eq!(json, "\"skill\"");
        let parsed: PackageType = serde_json::from_str("\"validator\"").unwrap();
        assert_eq!(parsed, PackageType::Validator);
        let json = serde_json::to_string(&PackageType::Tool).unwrap();
        assert_eq!(json, "\"tool\"");
        let parsed: PackageType = serde_json::from_str("\"plugin\"").unwrap();
        assert_eq!(parsed, PackageType::Plugin);
    }
}
