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

use super::types::{
    Rule, RuleFrontmatter, RuleSet, RuleSetManifest, Validator, ValidatorFrontmatter,
    ValidatorSource,
};

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

/// The path to the source code file patterns in the YAML include system.
const SOURCE_CODE_PATTERNS_PATH: &str = "file_groups/source_code";

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
    let mut frontmatter: ValidatorFrontmatter =
        serde_yaml::from_value(yaml_value).map_err(|e| AvpError::Validator {
            validator: path.display().to_string(),
            message: format!("failed to deserialize frontmatter: {}", e),
        })?;

    // Load source code patterns from expander for defaults
    let source_code_patterns: Option<Vec<String>> = expander.and_then(|exp| {
        exp.get(SOURCE_CODE_PATTERNS_PATH).and_then(|value| {
            value.as_sequence().map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
        })
    });

    // Apply sensible defaults (name from file stem, description, source code file patterns)
    frontmatter.apply_defaults(&path, source_code_patterns.as_deref());

    Ok(Validator {
        frontmatter,
        body: body.to_string(),
        source,
        path,
    })
}

/// Extract frontmatter and body from markdown content.
///
/// Returns (frontmatter, body). If no frontmatter is found (no `---` delimiters),
/// returns empty frontmatter and the entire content as body. Defaults will be
/// applied to create a valid validator.
fn extract_frontmatter<'a>(content: &'a str, _path: &Path) -> Result<(&'a str, &'a str), AvpError> {
    let content = content.trim();

    // If no frontmatter delimiter, return empty frontmatter and whole content as body
    if !content.starts_with("---") {
        return Ok(("", content));
    }

    // Find the closing ---
    let rest = &content[YAML_DELIMITER_LEN..];

    // If no closing delimiter, treat entire content as body with no frontmatter
    let Some(end_idx) = rest.find("\n---") else {
        return Ok(("", content));
    };

    let frontmatter = &rest[..end_idx].trim();
    let body = &rest[end_idx + YAML_CLOSING_DELIMITER_LEN..].trim();

    Ok((frontmatter, body))
}

// ============================================================================
// RuleSet Parsing Functions (New Architecture)
// ============================================================================

/// Parse a RuleSet manifest from VALIDATOR.md content.
///
/// # Format
///
/// ```markdown
/// ---
/// name: security-rules
/// description: Critical security validations
/// version: 1.0.0
/// trigger: PostToolUse
/// match:
///   tools: [Write, Edit]
///   files: ["@file_groups/source_code"]
/// severity: error
/// ---
///
/// # Security Rules RuleSet
///
/// Common security validations...
/// ```
///
/// # Arguments
///
/// * `content` - The full VALIDATOR.md file content
/// * `dir_path` - The RuleSet directory path (for error messages and defaults)
/// * `expander` - Optional YAML expander for `@` references
///
/// # Returns
///
/// A parsed `RuleSetManifest` or an error if parsing fails.
pub fn parse_ruleset_manifest<C: DirectoryConfig>(
    content: &str,
    dir_path: &Path,
    expander: Option<&YamlExpander<C>>,
) -> Result<RuleSetManifest, AvpError> {
    // Extract frontmatter and body (body is unused for manifest, but validates format)
    let (frontmatter_str, _body) = extract_frontmatter(content, dir_path)?;

    // Parse YAML frontmatter
    let mut yaml_value: serde_yaml::Value =
        serde_yaml::from_str(frontmatter_str).map_err(|e| AvpError::Validator {
            validator: format!("{}/VALIDATOR.md", dir_path.display()),
            message: format!("failed to parse YAML frontmatter: {}", e),
        })?;

    // Expand includes if an expander is provided
    if let Some(exp) = expander {
        yaml_value = exp.expand(yaml_value).map_err(|e| AvpError::Validator {
            validator: format!("{}/VALIDATOR.md", dir_path.display()),
            message: format!("failed to expand YAML includes: {}", e),
        })?;
    }

    // Deserialize to typed manifest
    let mut manifest: RuleSetManifest =
        serde_yaml::from_value(yaml_value).map_err(|e| AvpError::Validator {
            validator: format!("{}/VALIDATOR.md", dir_path.display()),
            message: format!("failed to deserialize manifest: {}", e),
        })?;

    // Apply defaults (name from directory, description, version)
    manifest.apply_defaults(dir_path);

    Ok(manifest)
}

/// Parse a rule from a rule file within a RuleSet.
///
/// # Format
///
/// ```markdown
/// ---
/// name: no-secrets
/// description: Detect hardcoded secrets, API keys, and credentials
/// severity: error
/// timeout: 60
/// ---
///
/// # No Secrets Rule
///
/// You are a security validator that checks for hardcoded secrets...
/// ```
///
/// # Arguments
///
/// * `content` - The full rule file content
/// * `path` - The path to the rule file (for error messages and defaults)
///
/// # Returns
///
/// A parsed `Rule` or an error if parsing fails.
pub fn parse_rule(content: &str, path: &Path) -> Result<Rule, AvpError> {
    // Extract frontmatter and body
    let (frontmatter_str, body) = extract_frontmatter(content, path)?;

    // Parse YAML frontmatter
    let yaml_value: serde_yaml::Value =
        serde_yaml::from_str(frontmatter_str).map_err(|e| AvpError::Validator {
            validator: path.display().to_string(),
            message: format!("failed to parse YAML frontmatter: {}", e),
        })?;

    // Deserialize to typed frontmatter
    let mut frontmatter: RuleFrontmatter =
        serde_yaml::from_value(yaml_value).map_err(|e| AvpError::Validator {
            validator: path.display().to_string(),
            message: format!("failed to deserialize frontmatter: {}", e),
        })?;

    // Apply defaults (name from file stem, description)
    frontmatter.apply_defaults(path);

    Ok(Rule {
        name: frontmatter.name,
        description: frontmatter.description,
        body: body.to_string(),
        severity: frontmatter.severity,
        timeout: frontmatter.timeout,
    })
}

/// Parse a complete RuleSet from a directory.
///
/// # Directory Structure
///
/// ```
/// ruleset-name/
/// ├── VALIDATOR.md      (required: manifest)
/// └── rules/            (required directory)
///     ├── rule1.md
///     ├── rule2.md
///     └── ...
/// ```
///
/// # Arguments
///
/// * `dir_path` - Path to the RuleSet directory
/// * `source` - Where this RuleSet came from (builtin, user, project)
/// * `expander` - Optional YAML expander for manifest `@` references
///
/// # Returns
///
/// A parsed `RuleSet` with manifest and all rules, or an error if:
/// - VALIDATOR.md is missing
/// - VALIDATOR.md is invalid
/// - rules/ directory is missing
/// - Any rule file is invalid
/// - Duplicate rule names are found
pub fn parse_ruleset_directory<C: DirectoryConfig>(
    dir_path: &Path,
    source: ValidatorSource,
    expander: Option<&YamlExpander<C>>,
) -> Result<RuleSet, AvpError> {
    // Verify directory exists
    if !dir_path.is_dir() {
        return Err(AvpError::Validator {
            validator: dir_path.display().to_string(),
            message: "not a directory".to_string(),
        });
    }

    // Load and parse VALIDATOR.md manifest
    let manifest_path = dir_path.join("VALIDATOR.md");
    if !manifest_path.exists() {
        return Err(AvpError::Validator {
            validator: dir_path.display().to_string(),
            message: "missing VALIDATOR.md manifest".to_string(),
        });
    }

    let manifest_content = std::fs::read_to_string(&manifest_path).map_err(|e| {
        AvpError::Validator {
            validator: manifest_path.display().to_string(),
            message: format!("failed to read VALIDATOR.md: {}", e),
        }
    })?;

    let manifest = parse_ruleset_manifest(&manifest_content, dir_path, expander)?;

    // Load rules from rules/ directory
    let rules_dir = dir_path.join("rules");
    if !rules_dir.exists() {
        return Err(AvpError::Validator {
            validator: dir_path.display().to_string(),
            message: "missing rules/ directory".to_string(),
        });
    }

    if !rules_dir.is_dir() {
        return Err(AvpError::Validator {
            validator: dir_path.display().to_string(),
            message: "rules/ is not a directory".to_string(),
        });
    }

    // Collect all .md files in rules/ directory
    let mut rules = Vec::new();
    let mut rule_names = std::collections::HashSet::new();

    let entries = std::fs::read_dir(&rules_dir).map_err(|e| AvpError::Validator {
        validator: rules_dir.display().to_string(),
        message: format!("failed to read rules/ directory: {}", e),
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| AvpError::Validator {
            validator: rules_dir.display().to_string(),
            message: format!("failed to read directory entry: {}", e),
        })?;

        let path = entry.path();

        // Skip non-files and non-.md files
        if !path.is_file() {
            continue;
        }

        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        // Skip partials
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with('_'))
            .unwrap_or(false)
        {
            continue;
        }

        // Parse the rule
        let rule_content = std::fs::read_to_string(&path).map_err(|e| AvpError::Validator {
            validator: path.display().to_string(),
            message: format!("failed to read rule file: {}", e),
        })?;

        let rule = parse_rule(&rule_content, &path)?;

        // Check for duplicate rule names
        if !rule_names.insert(rule.name.clone()) {
            return Err(AvpError::Validator {
                validator: dir_path.display().to_string(),
                message: format!("duplicate rule name '{}' in RuleSet", rule.name),
            });
        }

        rules.push(rule);
    }

    // Sort rules by name for deterministic ordering
    rules.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(RuleSet {
        manifest,
        rules,
        source,
        base_path: dir_path.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::HookType;
    use crate::validator::types::{Severity, DEFAULT_VALIDATOR_TIMEOUT_SECONDS};

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
    fn test_parse_validator_no_frontmatter_uses_defaults() {
        let content = "# My Validation Rule\n\nCheck that the code follows best practices.";

        let validator = parse_validator(
            content,
            PathBuf::from("/validators/code-quality.md"),
            ValidatorSource::User,
        )
        .unwrap();

        // Name defaults to file stem
        assert_eq!(validator.name(), "code-quality");
        // Description defaults based on name
        assert_eq!(validator.description(), "Validator: code-quality");
        // Trigger defaults to PostToolUse
        assert_eq!(validator.trigger(), HookType::PostToolUse);
        // Severity defaults to warn
        assert_eq!(validator.severity(), Severity::Warn);
        // Body is the entire content
        assert!(validator.body.contains("My Validation Rule"));
        assert!(validator
            .body
            .contains("Check that the code follows best practices"));
    }

    #[test]
    fn test_parse_validator_unclosed_frontmatter_uses_defaults() {
        // If someone starts with --- but forgets to close, treat whole content as body
        let content = r#"---
name: unclosed

This is actually the body since there's no closing delimiter.
"#;

        let validator = parse_validator(
            content,
            PathBuf::from("my-validator.md"),
            ValidatorSource::Project,
        )
        .unwrap();

        // Name defaults to file stem since frontmatter wasn't properly parsed
        assert_eq!(validator.name(), "my-validator");
        // Body contains the whole content
        assert!(validator.body.contains("name: unclosed"));
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
        // Default timeout
        assert_eq!(
            validator.frontmatter.timeout,
            DEFAULT_VALIDATOR_TIMEOUT_SECONDS
        );
        // Default once is false
        assert!(!validator.frontmatter.once);
        // No match criteria without expander (no source code patterns available)
        assert!(validator.frontmatter.match_criteria.is_none());
    }

    #[test]
    fn test_parse_validator_name_defaults_to_file_stem() {
        let content = r#"---
---

Minimal body.
"#;

        let validator = parse_validator(
            content,
            PathBuf::from("/path/to/my-custom-validator.md"),
            ValidatorSource::Project,
        )
        .unwrap();

        assert_eq!(validator.name(), "my-custom-validator");
    }

    #[test]
    fn test_parse_validator_description_defaults_from_name() {
        let content = r#"---
name: check-types
---

Body.
"#;

        let validator =
            parse_validator(content, PathBuf::from("test.md"), ValidatorSource::Builtin).unwrap();

        assert_eq!(validator.description(), "Validator: check-types");
    }

    #[test]
    fn test_parse_validator_trigger_defaults_to_post_tool_use() {
        let content = r#"---
---

Body.
"#;

        let validator =
            parse_validator(content, PathBuf::from("test.md"), ValidatorSource::Builtin).unwrap();

        assert_eq!(validator.trigger(), HookType::PostToolUse);
    }

    #[test]
    fn test_parse_validator_minimal_frontmatter() {
        // Validators can have completely empty frontmatter - all values will be defaulted
        let content = r#"---
---

Check that the code is correct.
"#;

        let validator = parse_validator(
            content,
            PathBuf::from("code-review.md"),
            ValidatorSource::User,
        )
        .unwrap();

        assert_eq!(validator.name(), "code-review");
        assert_eq!(validator.description(), "Validator: code-review");
        assert_eq!(validator.trigger(), HookType::PostToolUse);
        assert_eq!(validator.severity(), Severity::Warn);
        assert!(validator.body.contains("Check that the code is correct"));
    }

    #[test]
    fn test_parse_validator_with_expansion_applies_source_code_defaults() {
        use swissarmyhammer_directory::AvpConfig;

        let content = r#"---
name: test-validator
description: Test
---

Body.
"#;

        // Create expander with source code patterns
        let mut expander = YamlExpander::<AvpConfig>::new();
        expander
            .add_builtin(
                "file_groups/source_code",
                r#"
- "*.rs"
- "*.ts"
- "*.py"
"#,
            )
            .unwrap();

        let validator = parse_validator_with_expansion(
            content,
            PathBuf::from("test.md"),
            ValidatorSource::Builtin,
            &expander,
        )
        .unwrap();

        // Should have default match criteria from source code patterns
        let match_criteria = validator
            .frontmatter
            .match_criteria
            .as_ref()
            .expect("match_criteria should be set from defaults");

        assert!(match_criteria.tools.is_empty());
        assert!(match_criteria.files.contains(&"*.rs".to_string()));
        assert!(match_criteria.files.contains(&"*.ts".to_string()));
        assert!(match_criteria.files.contains(&"*.py".to_string()));
    }

    #[test]
    fn test_parse_validator_explicit_match_not_overridden() {
        use swissarmyhammer_directory::AvpConfig;

        let content = r#"---
name: bash-only
description: Only checks bash
match:
  tools:
    - Bash
  files:
    - "*.sh"
---

Body.
"#;

        // Create expander with source code patterns
        let mut expander = YamlExpander::<AvpConfig>::new();
        expander
            .add_builtin(
                "file_groups/source_code",
                r#"
- "*.rs"
- "*.ts"
"#,
            )
            .unwrap();

        let validator = parse_validator_with_expansion(
            content,
            PathBuf::from("test.md"),
            ValidatorSource::Builtin,
            &expander,
        )
        .unwrap();

        // Should preserve explicit match criteria, not use defaults
        let match_criteria = validator.frontmatter.match_criteria.as_ref().unwrap();
        assert_eq!(match_criteria.tools, vec!["Bash"]);
        assert_eq!(match_criteria.files, vec!["*.sh"]);
    }
}
