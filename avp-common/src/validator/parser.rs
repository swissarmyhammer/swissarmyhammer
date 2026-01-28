//! Parser for validator markdown files with YAML frontmatter.
//!
//! Validators are markdown files with a YAML frontmatter block delimited by `---`.
//! The frontmatter contains configuration, and the body contains validation instructions.
//!
//! # YAML Include Expansion
//!
//! Validator frontmatter supports `@path/to/file` references that expand to the
//! contents of YAML files. Use `parse_validator_with_expansion` to enable this.
//!
//! ## Example
//!
//! ```yaml
//! ---
//! name: no-secrets
//! match:
//!   files:
//!     - "@file_groups/source_code"
//! ---
//! ```

use std::path::{Path, PathBuf};

use swissarmyhammer_directory::{DirectoryConfig, YamlExpander};

use crate::error::AvpError;

use super::types::{Validator, ValidatorFrontmatter, ValidatorSource};

/// Length of the YAML frontmatter opening delimiter "---".
const YAML_DELIMITER_LEN: usize = 3;

/// Length of the closing YAML delimiter "\n---" (newline + delimiter).
const YAML_CLOSING_DELIMITER_LEN: usize = 4;

/// Parse a validator from markdown content with YAML frontmatter.
///
/// # Format
///
/// ```markdown
/// ---
/// name: no-secrets
/// description: Detect hardcoded secrets in code
/// severity: error
/// trigger: PostToolUse
/// match:
///   tools: [Write, Edit]
///   files: ["*.ts", "*.js"]
/// ---
///
/// # No Secrets Validator
///
/// Instructions for the validation agent...
/// ```
///
/// # Arguments
///
/// * `content` - The full markdown file content
/// * `path` - The path to the validator file (for error messages)
/// * `source` - Where this validator came from (builtin, user, project)
///
/// # Returns
///
/// A parsed `Validator` or an error if parsing fails.
pub fn parse_validator(
    content: &str,
    path: PathBuf,
    source: ValidatorSource,
) -> Result<Validator, AvpError> {
    parse_validator_internal(
        content,
        path,
        source,
        None::<&YamlExpander<swissarmyhammer_directory::AvpConfig>>,
    )
}

/// Parse a validator with `@` include expansion.
///
/// This is like `parse_validator` but expands `@path/to/file` references
/// in the YAML frontmatter using the provided expander.
///
/// # Example
///
/// ```yaml
/// ---
/// name: no-secrets
/// match:
///   files:
///     - "@file_groups/source_code"
///     - "*.custom"
/// ---
/// ```
///
/// The `@file_groups/source_code` will be expanded to the contents of
/// `file_groups/source_code.yaml`.
pub fn parse_validator_with_expansion<C: DirectoryConfig>(
    content: &str,
    path: PathBuf,
    source: ValidatorSource,
    expander: &YamlExpander<C>,
) -> Result<Validator, AvpError> {
    parse_validator_internal(content, path, source, Some(expander))
}

/// Internal implementation that optionally expands includes.
fn parse_validator_internal<C: DirectoryConfig>(
    content: &str,
    path: PathBuf,
    source: ValidatorSource,
    expander: Option<&YamlExpander<C>>,
) -> Result<Validator, AvpError> {
    // Skip partials - they're template includes, not validators
    // Identified by _partials/ in path or {% partial %} marker in content
    if path.to_string_lossy().contains("_partials/")
        || content.trim_start().starts_with("{% partial %}")
    {
        return Err(AvpError::Partial(path.display().to_string()));
    }

    // Split on frontmatter delimiters
    let (frontmatter_str, body) = extract_frontmatter(content, &path)?;

    // Parse YAML frontmatter
    let mut yaml_value: serde_yaml::Value =
        serde_yaml::from_str(frontmatter_str).map_err(|e| AvpError::Validator {
            validator: path.display().to_string(),
            message: format!("failed to parse YAML frontmatter: {}", e),
        })?;

    // Expand includes if an expander is provided
    if let Some(exp) = expander {
        yaml_value = exp.expand(yaml_value).map_err(|e| AvpError::Validator {
            validator: path.display().to_string(),
            message: format!("failed to expand YAML includes: {}", e),
        })?;
    }

    // Deserialize to typed frontmatter
    let frontmatter: ValidatorFrontmatter =
        serde_yaml::from_value(yaml_value).map_err(|e| AvpError::Validator {
            validator: path.display().to_string(),
            message: format!("failed to deserialize frontmatter: {}", e),
        })?;

    Ok(Validator {
        frontmatter,
        body: body.to_string(),
        source,
        path,
    })
}

/// Extract frontmatter and body from markdown content.
///
/// Returns (frontmatter, body) or an error if no frontmatter is found.
fn extract_frontmatter<'a>(content: &'a str, path: &Path) -> Result<(&'a str, &'a str), AvpError> {
    let content = content.trim();

    // Must start with ---
    if !content.starts_with("---") {
        return Err(AvpError::Validator {
            validator: path.display().to_string(),
            message: "file must start with YAML frontmatter (---)".to_string(),
        });
    }

    // Find the closing ---
    let rest = &content[YAML_DELIMITER_LEN..];
    let end_idx = rest.find("\n---").ok_or_else(|| AvpError::Validator {
        validator: path.display().to_string(),
        message: "missing closing frontmatter delimiter (---)".to_string(),
    })?;

    let frontmatter = &rest[..end_idx].trim();
    let body = &rest[end_idx + YAML_CLOSING_DELIMITER_LEN..].trim();

    Ok((frontmatter, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::HookType;
    use crate::validator::types::Severity;

    #[test]
    fn test_parse_validator_basic() {
        let content = r#"---
name: test-validator
description: A test validator
severity: error
trigger: PreToolUse
---

# Test Validator

This is the body of the validator.
"#;

        let validator =
            parse_validator(content, PathBuf::from("test.md"), ValidatorSource::Builtin).unwrap();

        assert_eq!(validator.name(), "test-validator");
        assert_eq!(validator.description(), "A test validator");
        assert_eq!(validator.severity(), Severity::Error);
        assert_eq!(validator.trigger(), HookType::PreToolUse);
        assert!(validator.body.contains("This is the body"));
    }

    #[test]
    fn test_parse_validator_with_match() {
        let content = r#"---
name: file-validator
description: Validates specific files
severity: warn
trigger: PostToolUse
match:
  tools:
    - Write
    - Edit
  files:
    - "*.ts"
    - "src/**/*.rs"
---

Body content.
"#;

        let validator =
            parse_validator(content, PathBuf::from("test.md"), ValidatorSource::User).unwrap();

        let match_criteria = validator.frontmatter.match_criteria.as_ref().unwrap();
        assert_eq!(match_criteria.tools, vec!["Write", "Edit"]);
        assert_eq!(match_criteria.files, vec!["*.ts", "src/**/*.rs"]);
    }

    #[test]
    fn test_parse_validator_with_tags() {
        let content = r#"---
name: tagged-validator
description: Has tags
severity: info
trigger: SessionStart
tags:
  - blocking
  - secrets
  - security
---

Body.
"#;

        let validator =
            parse_validator(content, PathBuf::from("test.md"), ValidatorSource::Project).unwrap();

        assert_eq!(
            validator.frontmatter.tags,
            vec!["blocking", "secrets", "security"]
        );
    }

    #[test]
    fn test_parse_validator_missing_frontmatter() {
        let content = "# No frontmatter\n\nJust body.";

        let result = parse_validator(content, PathBuf::from("test.md"), ValidatorSource::Builtin);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("must start with YAML frontmatter"));
    }

    #[test]
    fn test_parse_validator_unclosed_frontmatter() {
        let content = r#"---
name: unclosed
description: Missing closing delimiter
severity: error
trigger: PreToolUse

Body without closing delimiter.
"#;

        let result = parse_validator(content, PathBuf::from("test.md"), ValidatorSource::Builtin);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("missing closing frontmatter"));
    }

    #[test]
    fn test_parse_validator_invalid_yaml() {
        let content = r#"---
name: [invalid yaml
description: broken
---

Body.
"#;

        let result = parse_validator(content, PathBuf::from("test.md"), ValidatorSource::Builtin);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("failed to parse YAML"));
    }

    #[test]
    fn test_parse_validator_default_values() {
        let content = r#"---
name: minimal
description: Minimal validator
trigger: PreToolUse
---

Body.
"#;

        let validator =
            parse_validator(content, PathBuf::from("test.md"), ValidatorSource::Builtin).unwrap();

        // Default severity is warn
        assert_eq!(validator.severity(), Severity::Warn);
        // Default timeout is 30
        assert_eq!(validator.frontmatter.timeout, 30);
        // Default once is false
        assert!(!validator.frontmatter.once);
        // No match criteria by default
        assert!(validator.frontmatter.match_criteria.is_none());
    }
}
