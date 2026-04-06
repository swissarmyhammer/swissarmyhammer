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

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use swissarmyhammer_directory::{DirectoryConfig, YamlExpander};
use swissarmyhammer_templating::TemplateEngine;

use crate::error::AvpError;

use super::types::{
    Rule, RuleFrontmatter, RuleSet, RuleSetManifest, Validator, ValidatorFrontmatter,
    ValidatorSource,
};

/// Length of the YAML frontmatter opening delimiter "---".
const YAML_DELIMITER_LEN: usize = 3;

/// Length of the closing YAML delimiter "\n---" (newline + delimiter).
const YAML_CLOSING_DELIMITER_LEN: usize = 4;

/// Shared template engine for frontmatter rendering.
static TEMPLATE_ENGINE: LazyLock<TemplateEngine> = LazyLock::new(TemplateEngine::new);

/// Standard Liquid template variables available in all frontmatter.
///
/// Currently provides:
/// - `version` — AVP workspace version from Cargo.toml
fn frontmatter_vars() -> HashMap<String, String> {
    let mut vars = HashMap::new();
    vars.insert("version".to_string(), crate::VERSION.to_string());
    vars
}

/// Render Liquid template variables in a frontmatter string.
///
/// Falls back to the original string if rendering fails (e.g. the
/// frontmatter contains non-template `{` characters that confuse Liquid).
fn render_frontmatter(frontmatter: &str) -> String {
    // Skip rendering if there are no template markers at all
    if !frontmatter.contains("{{") {
        return frontmatter.to_string();
    }
    TEMPLATE_ENGINE
        .render(frontmatter, &frontmatter_vars())
        .unwrap_or_else(|_| frontmatter.to_string())
}

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

    // Render Liquid template variables before YAML parsing
    let rendered = render_frontmatter(frontmatter_str);

    // Parse YAML frontmatter
    let mut yaml_value: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&rendered).map_err(|e| AvpError::Validator {
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
        serde_yaml_ng::from_value(yaml_value).map_err(|e| AvpError::Validator {
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

    // Render Liquid template variables before YAML parsing
    let rendered = render_frontmatter(frontmatter_str);

    // Parse YAML frontmatter
    let mut yaml_value: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&rendered).map_err(|e| AvpError::Validator {
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
        serde_yaml_ng::from_value(yaml_value).map_err(|e| AvpError::Validator {
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

    // Render Liquid template variables before YAML parsing
    let rendered = render_frontmatter(frontmatter_str);

    // Parse YAML frontmatter
    let yaml_value: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&rendered).map_err(|e| AvpError::Validator {
            validator: path.display().to_string(),
            message: format!("failed to parse YAML frontmatter: {}", e),
        })?;

    // Deserialize to typed frontmatter
    let mut frontmatter: RuleFrontmatter =
        serde_yaml_ng::from_value(yaml_value).map_err(|e| AvpError::Validator {
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
/// ```text
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

    let manifest_content =
        std::fs::read_to_string(&manifest_path).map_err(|e| AvpError::Validator {
            validator: manifest_path.display().to_string(),
            message: format!("failed to read VALIDATOR.md: {}", e),
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

    // ── Liquid template rendering tests ──────────────────────────────

    #[test]
    fn test_render_frontmatter_version_variable() {
        let input = r#"name: test
version: "{{ version }}""#;
        let result = render_frontmatter(input);
        assert_eq!(
            result,
            format!("name: test\nversion: \"{}\"", crate::VERSION)
        );
    }

    #[test]
    fn test_render_frontmatter_no_templates_passthrough() {
        let input = "name: test\nversion: 1.0.0";
        let result = render_frontmatter(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_render_frontmatter_invalid_template_passthrough() {
        // Invalid Liquid should fall back to the original string
        let input = "name: {{ invalid | nonexistent_filter }}";
        let result = render_frontmatter(input);
        // Should not panic — returns original or rendered (filter may be ignored)
        assert!(!result.is_empty());
    }

    #[test]
    fn test_parse_ruleset_manifest_with_version_template() {
        let content = r#"---
name: test-ruleset
description: Test
metadata:
  version: "{{ version }}"
trigger: PostToolUse
---

# Test
"#;
        let manifest = parse_ruleset_manifest::<swissarmyhammer_directory::AvpConfig>(
            content,
            Path::new("test"),
            None,
        )
        .unwrap();
        assert_eq!(manifest.metadata.version, crate::VERSION);
    }

    #[test]
    fn test_parse_rule_with_version_template() {
        let content = r#"---
name: test-rule
description: "Rule at {{ version }}"
---

Body.
"#;
        let rule = parse_rule(content, Path::new("test-rule.md")).unwrap();
        assert_eq!(rule.description, format!("Rule at {}", crate::VERSION));
    }

    #[test]
    fn test_parse_validator_with_version_template() {
        let content = r#"---
name: test-validator
description: "Validator {{ version }}"
---

Body.
"#;
        let validator = parse_validator(
            content,
            PathBuf::from("test-validator.md"),
            ValidatorSource::Builtin,
        )
        .unwrap();
        assert_eq!(
            validator.frontmatter.description,
            format!("Validator {}", crate::VERSION)
        );
    }

    #[test]
    fn test_frontmatter_vars_contains_version() {
        let vars = frontmatter_vars();
        assert_eq!(vars.get("version").unwrap(), crate::VERSION);
    }

    // =========================================================================
    // RuleSet Directory Parsing Tests
    // =========================================================================

    #[test]
    fn test_parse_ruleset_manifest_basic() {
        let content = r#"---
name: security-rules
description: Critical security validations
trigger: PostToolUse
severity: error
---

# Security Rules
"#;
        let manifest = parse_ruleset_manifest::<swissarmyhammer_directory::AvpConfig>(
            content,
            std::path::Path::new("security-rules"),
            None,
        )
        .unwrap();
        assert_eq!(manifest.name, "security-rules");
        assert_eq!(manifest.description, "Critical security validations");
        assert_eq!(manifest.trigger, HookType::PostToolUse);
        assert_eq!(manifest.severity, Severity::Error);
    }

    #[test]
    fn test_parse_ruleset_manifest_defaults() {
        let content = r#"---
name: ""
description: ""
---

Minimal manifest.
"#;
        let manifest = parse_ruleset_manifest::<swissarmyhammer_directory::AvpConfig>(
            content,
            std::path::Path::new("my-rules"),
            None,
        )
        .unwrap();
        // Name defaults to directory name when empty
        assert_eq!(manifest.name, "my-rules");
        // Description defaults
        assert_eq!(manifest.description, "RuleSet: my-rules");
        // Version defaults to 1.0.0
        assert_eq!(manifest.metadata.version, "1.0.0");
        // Trigger defaults to PostToolUse
        assert_eq!(manifest.trigger, HookType::PostToolUse);
        // Severity defaults to Warn
        assert_eq!(manifest.severity, Severity::Warn);
    }

    #[test]
    fn test_parse_rule_basic() {
        let content = r#"---
name: no-secrets
description: Detect hardcoded secrets
severity: error
timeout: 60
---

Check for API keys, passwords, and tokens.
"#;
        let rule = parse_rule(content, std::path::Path::new("no-secrets.md")).unwrap();
        assert_eq!(rule.name, "no-secrets");
        assert_eq!(rule.description, "Detect hardcoded secrets");
        assert_eq!(rule.severity, Some(Severity::Error));
        assert_eq!(rule.timeout, Some(60));
        assert!(rule.body.contains("Check for API keys"));
    }

    #[test]
    fn test_parse_rule_defaults() {
        let content = r#"---
name: ""
description: ""
---

Check the code.
"#;
        let rule = parse_rule(content, std::path::Path::new("check-code.md")).unwrap();
        assert_eq!(rule.name, "check-code");
        assert_eq!(rule.description, "Rule: check-code");
        assert!(rule.severity.is_none());
        assert!(rule.timeout.is_none());
    }

    #[test]
    fn test_parse_rule_with_all_fields() {
        let content = r#"---
name: my-rule
description: My custom rule
severity: info
timeout: 120
---

Body content here.
"#;
        let rule = parse_rule(content, std::path::Path::new("my-rule.md")).unwrap();
        assert_eq!(rule.name, "my-rule");
        assert_eq!(rule.description, "My custom rule");
        assert_eq!(rule.severity, Some(Severity::Info));
        assert_eq!(rule.timeout, Some(120));
    }

    #[test]
    fn test_parse_ruleset_directory_complete() {
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("my-ruleset");
        std::fs::create_dir_all(dir.join("rules")).unwrap();

        // Write VALIDATOR.md
        std::fs::write(
            dir.join("VALIDATOR.md"),
            r#"---
name: test-ruleset
description: Test RuleSet
trigger: PreToolUse
severity: error
---

# Test RuleSet
"#,
        )
        .unwrap();

        // Write rule files
        std::fs::write(
            dir.join("rules/rule-a.md"),
            r#"---
name: rule-a
description: First rule
---

Check first thing.
"#,
        )
        .unwrap();

        std::fs::write(
            dir.join("rules/rule-b.md"),
            r#"---
name: rule-b
description: Second rule
severity: warn
---

Check second thing.
"#,
        )
        .unwrap();

        let ruleset = parse_ruleset_directory::<swissarmyhammer_directory::AvpConfig>(
            &dir,
            ValidatorSource::Project,
            None,
        )
        .unwrap();

        assert_eq!(ruleset.manifest.name, "test-ruleset");
        assert_eq!(ruleset.rules.len(), 2);
        // Rules should be sorted by name
        assert_eq!(ruleset.rules[0].name, "rule-a");
        assert_eq!(ruleset.rules[1].name, "rule-b");
        assert_eq!(ruleset.source, ValidatorSource::Project);
    }

    #[test]
    fn test_parse_ruleset_directory_not_a_directory() {
        let result = parse_ruleset_directory::<swissarmyhammer_directory::AvpConfig>(
            std::path::Path::new("/nonexistent/path"),
            ValidatorSource::Builtin,
            None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a directory"));
    }

    #[test]
    fn test_parse_ruleset_directory_missing_manifest() {
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("no-manifest");
        std::fs::create_dir_all(&dir).unwrap();

        let result = parse_ruleset_directory::<swissarmyhammer_directory::AvpConfig>(
            &dir,
            ValidatorSource::Builtin,
            None,
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("missing VALIDATOR.md"));
    }

    #[test]
    fn test_parse_ruleset_directory_missing_rules_dir() {
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("no-rules");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("VALIDATOR.md"),
            "---\nname: test\ndescription: Test\n---\nBody.",
        )
        .unwrap();

        let result = parse_ruleset_directory::<swissarmyhammer_directory::AvpConfig>(
            &dir,
            ValidatorSource::Builtin,
            None,
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("missing rules/ directory"));
    }

    #[test]
    fn test_parse_ruleset_directory_duplicate_rule_names() {
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("dup-rules");
        std::fs::create_dir_all(dir.join("rules")).unwrap();
        std::fs::write(
            dir.join("VALIDATOR.md"),
            "---\nname: test\ndescription: Test\n---\nBody.",
        )
        .unwrap();

        // Two files with the same rule name
        std::fs::write(
            dir.join("rules/first.md"),
            "---\nname: duplicate\ndescription: Dup1\n---\nBody1.",
        )
        .unwrap();
        std::fs::write(
            dir.join("rules/second.md"),
            "---\nname: duplicate\ndescription: Dup2\n---\nBody2.",
        )
        .unwrap();

        let result = parse_ruleset_directory::<swissarmyhammer_directory::AvpConfig>(
            &dir,
            ValidatorSource::Builtin,
            None,
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("duplicate rule name"));
    }

    #[test]
    fn test_parse_ruleset_directory_skips_non_md_files() {
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("skip-non-md");
        std::fs::create_dir_all(dir.join("rules")).unwrap();
        std::fs::write(
            dir.join("VALIDATOR.md"),
            "---\nname: test\ndescription: Test\n---\nBody.",
        )
        .unwrap();

        // Write a .md file and a non-.md file
        std::fs::write(
            dir.join("rules/valid.md"),
            "---\nname: valid\ndescription: Valid\n---\nBody.",
        )
        .unwrap();
        std::fs::write(dir.join("rules/ignored.txt"), "Not a rule").unwrap();

        let ruleset = parse_ruleset_directory::<swissarmyhammer_directory::AvpConfig>(
            &dir,
            ValidatorSource::Builtin,
            None,
        )
        .unwrap();

        assert_eq!(ruleset.rules.len(), 1);
        assert_eq!(ruleset.rules[0].name, "valid");
    }

    #[test]
    fn test_parse_ruleset_directory_skips_partial_files() {
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("skip-partials");
        std::fs::create_dir_all(dir.join("rules")).unwrap();
        std::fs::write(
            dir.join("VALIDATOR.md"),
            "---\nname: test\ndescription: Test\n---\nBody.",
        )
        .unwrap();

        std::fs::write(
            dir.join("rules/valid.md"),
            "---\nname: valid\ndescription: Valid\n---\nBody.",
        )
        .unwrap();
        // Files starting with _ are skipped as partials
        std::fs::write(
            dir.join("rules/_partial.md"),
            "---\nname: partial\ndescription: Partial\n---\nBody.",
        )
        .unwrap();

        let ruleset = parse_ruleset_directory::<swissarmyhammer_directory::AvpConfig>(
            &dir,
            ValidatorSource::Builtin,
            None,
        )
        .unwrap();

        assert_eq!(ruleset.rules.len(), 1);
    }

    #[test]
    fn test_parse_validator_skips_partials_by_path() {
        let content = "---\nname: test\n---\nBody.";
        let result = parse_validator(
            content,
            PathBuf::from("/validators/_partials/common.md"),
            ValidatorSource::Builtin,
        );
        assert!(result.is_err());
        // Should be a Partial error
        let err = result.unwrap_err();
        assert!(err.is_partial());
    }

    #[test]
    fn test_parse_validator_skips_partials_by_content() {
        let content = "{% partial %}\n\nThis is a partial.";
        let result = parse_validator(
            content,
            PathBuf::from("some-validator.md"),
            ValidatorSource::Builtin,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ruleset_manifest_invalid_yaml() {
        let content = r#"---
name: [invalid yaml
---

Body.
"#;
        let result = parse_ruleset_manifest::<swissarmyhammer_directory::AvpConfig>(
            content,
            std::path::Path::new("test"),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_rule_invalid_yaml() {
        let content = r#"---
name: [invalid
---

Body.
"#;
        let result = parse_rule(content, std::path::Path::new("test.md"));
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_frontmatter_empty_content() {
        let (frontmatter, body) = extract_frontmatter("", std::path::Path::new("test")).unwrap();
        assert_eq!(frontmatter, "");
        assert_eq!(body, "");
    }

    #[test]
    fn test_extract_frontmatter_no_delimiters() {
        let content = "Just body content.";
        let (frontmatter, body) =
            extract_frontmatter(content, std::path::Path::new("test")).unwrap();
        assert_eq!(frontmatter, "");
        assert_eq!(body, "Just body content.");
    }

    #[test]
    fn test_extract_frontmatter_unclosed() {
        let content = "---\nname: test\nNo closing delimiter.";
        let (frontmatter, body) =
            extract_frontmatter(content, std::path::Path::new("test")).unwrap();
        assert_eq!(frontmatter, "");
        assert!(body.contains("name: test"));
    }

    #[test]
    fn test_parse_ruleset_manifest_with_expansion() {
        use swissarmyhammer_directory::AvpConfig;

        let content = r#"---
name: expanded
description: Test expansion
match:
  files:
    - "@file_groups/source_code"
---

Body.
"#;

        let mut expander = swissarmyhammer_directory::YamlExpander::<AvpConfig>::new();
        expander
            .add_builtin(
                "file_groups/source_code",
                r#"
- "*.rs"
- "*.ts"
"#,
            )
            .unwrap();

        let manifest =
            parse_ruleset_manifest(content, std::path::Path::new("test"), Some(&expander)).unwrap();

        assert_eq!(manifest.name, "expanded");
        let match_criteria = manifest.match_criteria.unwrap();
        assert!(match_criteria.files.contains(&"*.rs".to_string()));
        assert!(match_criteria.files.contains(&"*.ts".to_string()));
    }
}
