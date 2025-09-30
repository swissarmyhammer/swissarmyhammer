//! Rule definitions and core types
//!
//! This module defines the `Rule` struct which represents a validation rule
//! for checking code quality, security, documentation, and other aspects.
//!
//! # Example
//!
//! ```
//! use swissarmyhammer_rules::{Rule, Severity};
//! use std::collections::HashMap;
//!
//! let rule = Rule::new(
//!     "no-hardcoded-secrets".to_string(),
//!     "Check for hardcoded API keys".to_string(),
//!     Severity::Error,
//! );
//!
//! assert_eq!(rule.name, "no-hardcoded-secrets");
//! assert_eq!(rule.severity, Severity::Error);
//! assert!(!rule.auto_fix);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::{Result, Severity};
use swissarmyhammer_common::SwissArmyHammerError;

/// Represents a validation rule
///
/// Rules are similar to prompts but have no parameters and include a severity field.
/// Rules check code/artifacts and report violations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    /// Name of the rule (e.g., "no-hardcoded-secrets")
    pub name: String,

    /// The rule content (checking instructions)
    pub template: String,

    /// Optional description of what the rule checks
    pub description: Option<String>,

    /// Optional category (e.g., "security", "code-quality")
    pub category: Option<String>,

    /// Tags for filtering and organization
    #[serde(default)]
    pub tags: Vec<String>,

    /// Source file path where the rule was loaded from
    #[serde(skip)]
    pub source: Option<PathBuf>,

    /// Additional metadata as key-value pairs
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Severity level of violations
    pub severity: Severity,

    /// Whether rule can auto-fix violations (future feature)
    #[serde(default)]
    pub auto_fix: bool,
}

impl Rule {
    /// Create a new rule with the minimum required fields
    ///
    /// # Arguments
    ///
    /// * `name` - The rule identifier
    /// * `template` - The rule checking instructions
    /// * `severity` - The severity level for violations
    ///
    /// # Example
    ///
    /// ```
    /// use swissarmyhammer_rules::{Rule, Severity};
    ///
    /// let rule = Rule::new(
    ///     "test-rule".to_string(),
    ///     "Check something".to_string(),
    ///     Severity::Warning,
    /// );
    /// ```
    pub fn new(name: String, template: String, severity: Severity) -> Self {
        Self {
            name,
            template,
            description: None,
            category: None,
            tags: Vec::new(),
            source: None,
            metadata: HashMap::new(),
            severity,
            auto_fix: false,
        }
    }

    /// Check if this rule is a partial template
    ///
    /// Partial templates start with `{% partial %}` and are used for
    /// inclusion in other rules, not for direct checking.
    ///
    /// # Example
    ///
    /// ```
    /// use swissarmyhammer_rules::{Rule, Severity};
    ///
    /// let partial = Rule::new(
    ///     "common-patterns".to_string(),
    ///     "{% partial %}\nSome shared content".to_string(),
    ///     Severity::Info,
    /// );
    /// assert!(partial.is_partial());
    ///
    /// let normal = Rule::new(
    ///     "check-something".to_string(),
    ///     "Check for issues".to_string(),
    ///     Severity::Error,
    /// );
    /// assert!(!normal.is_partial());
    /// ```
    pub fn is_partial(&self) -> bool {
        self.template.trim_start().starts_with("{% partial %}")
    }

    /// Validate that the rule is well-formed
    ///
    /// Checks:
    /// - Name is not empty
    /// - Template is not empty
    /// - If partial, template must start with `{% partial %}`
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails
    ///
    /// # Example
    ///
    /// ```
    /// use swissarmyhammer_rules::{Rule, Severity};
    ///
    /// let rule = Rule::new(
    ///     "valid-rule".to_string(),
    ///     "Check something".to_string(),
    ///     Severity::Error,
    /// );
    /// assert!(rule.validate().is_ok());
    ///
    /// let invalid = Rule::new(
    ///     "".to_string(),
    ///     "template".to_string(),
    ///     Severity::Error,
    /// );
    /// assert!(invalid.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<()> {
        // Check name is not empty
        if self.name.is_empty() {
            return Err(SwissArmyHammerError::Other {
                message: "Rule name cannot be empty".to_string(),
            });
        }

        // Check template is not empty
        if self.template.is_empty() {
            return Err(SwissArmyHammerError::Other {
                message: "Rule template cannot be empty".to_string(),
            });
        }

        // If template starts with {% partial %}, ensure it's properly formatted
        let trimmed = self.template.trim_start();
        if trimmed.starts_with("{% partial %}") {
            // Partial is valid if it has content after the marker
            if trimmed == "{% partial %}" {
                return Err(SwissArmyHammerError::Other {
                    message: "Partial template must have content after {% partial %} marker"
                        .to_string(),
                });
            }
        }

        Ok(())
    }

    /// Create a builder for constructing rules with optional fields
    pub fn builder(name: String, template: String, severity: Severity) -> RuleBuilder {
        RuleBuilder {
            name,
            template,
            severity,
            description: None,
            category: None,
            tags: Vec::new(),
            source: None,
            metadata: HashMap::new(),
            auto_fix: false,
        }
    }
}

/// Builder for constructing rules with optional fields
///
/// # Example
///
/// ```
/// use swissarmyhammer_rules::{Rule, Severity};
///
/// let rule = Rule::builder(
///     "test-rule".to_string(),
///     "Check something".to_string(),
///     Severity::Error,
/// )
/// .description("A test rule".to_string())
/// .category("testing".to_string())
/// .tag("test".to_string())
/// .tag("example".to_string())
/// .build();
///
/// assert_eq!(rule.name, "test-rule");
/// assert_eq!(rule.description, Some("A test rule".to_string()));
/// assert_eq!(rule.category, Some("testing".to_string()));
/// assert_eq!(rule.tags.len(), 2);
/// ```
pub struct RuleBuilder {
    name: String,
    template: String,
    severity: Severity,
    description: Option<String>,
    category: Option<String>,
    tags: Vec<String>,
    source: Option<PathBuf>,
    metadata: HashMap<String, serde_json::Value>,
    auto_fix: bool,
}

impl RuleBuilder {
    /// Set the description
    pub fn description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Set the category
    pub fn category(mut self, category: String) -> Self {
        self.category = Some(category);
        self
    }

    /// Add a tag
    pub fn tag(mut self, tag: String) -> Self {
        self.tags.push(tag);
        self
    }

    /// Set the source path
    pub fn source(mut self, source: PathBuf) -> Self {
        self.source = Some(source);
        self
    }

    /// Add metadata
    pub fn metadata_value(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Set auto_fix flag
    pub fn auto_fix(mut self, auto_fix: bool) -> Self {
        self.auto_fix = auto_fix;
        self
    }

    /// Build the rule
    pub fn build(self) -> Rule {
        Rule {
            name: self.name,
            template: self.template,
            description: self.description,
            category: self.category,
            tags: self.tags,
            source: self.source,
            metadata: self.metadata,
            severity: self.severity,
            auto_fix: self.auto_fix,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_new() {
        let rule = Rule::new(
            "test-rule".to_string(),
            "Check for issues".to_string(),
            Severity::Error,
        );

        assert_eq!(rule.name, "test-rule");
        assert_eq!(rule.template, "Check for issues");
        assert_eq!(rule.severity, Severity::Error);
        assert_eq!(rule.description, None);
        assert_eq!(rule.category, None);
        assert!(rule.tags.is_empty());
        assert_eq!(rule.source, None);
        assert!(rule.metadata.is_empty());
        assert!(!rule.auto_fix);
    }

    #[test]
    fn test_rule_is_partial() {
        let partial = Rule::new(
            "partial".to_string(),
            "{% partial %}\nContent here".to_string(),
            Severity::Info,
        );
        assert!(partial.is_partial());

        let partial_with_whitespace = Rule::new(
            "partial2".to_string(),
            "  {% partial %}\nContent".to_string(),
            Severity::Info,
        );
        assert!(partial_with_whitespace.is_partial());

        let normal = Rule::new(
            "normal".to_string(),
            "Normal template".to_string(),
            Severity::Error,
        );
        assert!(!normal.is_partial());

        let not_partial = Rule::new(
            "not-partial".to_string(),
            "Some text {% partial %}".to_string(),
            Severity::Warning,
        );
        assert!(!not_partial.is_partial());
    }

    #[test]
    fn test_rule_validate_success() {
        let rule = Rule::new(
            "valid-rule".to_string(),
            "Valid template".to_string(),
            Severity::Error,
        );
        assert!(rule.validate().is_ok());

        let partial = Rule::new(
            "valid-partial".to_string(),
            "{% partial %}\nContent".to_string(),
            Severity::Info,
        );
        assert!(partial.validate().is_ok());
    }

    #[test]
    fn test_rule_validate_empty_name() {
        let rule = Rule::new("".to_string(), "Template".to_string(), Severity::Error);
        let result = rule.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("name cannot be empty"));
    }

    #[test]
    fn test_rule_validate_empty_template() {
        let rule = Rule::new("rule-name".to_string(), "".to_string(), Severity::Error);
        let result = rule.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("template cannot be empty"));
    }

    #[test]
    fn test_rule_validate_empty_partial() {
        let rule = Rule::new(
            "empty-partial".to_string(),
            "{% partial %}".to_string(),
            Severity::Info,
        );
        let result = rule.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must have content after"));
    }

    #[test]
    fn test_rule_builder() {
        let rule = Rule::builder(
            "builder-rule".to_string(),
            "Template content".to_string(),
            Severity::Warning,
        )
        .description("Test description".to_string())
        .category("test-category".to_string())
        .tag("tag1".to_string())
        .tag("tag2".to_string())
        .auto_fix(true)
        .build();

        assert_eq!(rule.name, "builder-rule");
        assert_eq!(rule.template, "Template content");
        assert_eq!(rule.severity, Severity::Warning);
        assert_eq!(rule.description, Some("Test description".to_string()));
        assert_eq!(rule.category, Some("test-category".to_string()));
        assert_eq!(rule.tags, vec!["tag1".to_string(), "tag2".to_string()]);
        assert!(rule.auto_fix);
    }

    #[test]
    fn test_rule_serialization() {
        let rule = Rule::builder(
            "serialization-test".to_string(),
            "Check serialization".to_string(),
            Severity::Error,
        )
        .description("Test rule".to_string())
        .category("testing".to_string())
        .tag("test".to_string())
        .build();

        let json = serde_json::to_string(&rule).unwrap();
        let deserialized: Rule = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, rule.name);
        assert_eq!(deserialized.template, rule.template);
        assert_eq!(deserialized.severity, rule.severity);
        assert_eq!(deserialized.description, rule.description);
        assert_eq!(deserialized.category, rule.category);
        assert_eq!(deserialized.tags, rule.tags);
        assert_eq!(deserialized.auto_fix, rule.auto_fix);
    }

    #[test]
    fn test_rule_metadata() {
        use serde_json::json;

        let mut rule = Rule::new(
            "metadata-test".to_string(),
            "Test template".to_string(),
            Severity::Info,
        );

        rule.metadata.insert("key1".to_string(), json!("value1"));
        rule.metadata.insert("key2".to_string(), json!(42));

        assert_eq!(rule.metadata.get("key1"), Some(&json!("value1")));
        assert_eq!(rule.metadata.get("key2"), Some(&json!(42)));
    }

    #[test]
    fn test_rule_source_not_serialized() {
        let rule = Rule::builder(
            "source-test".to_string(),
            "Test".to_string(),
            Severity::Error,
        )
        .source(PathBuf::from("/path/to/rule.md"))
        .build();

        let json = serde_json::to_string(&rule).unwrap();
        // Source should not appear in JSON
        assert!(!json.contains("/path/to/rule.md"));
    }
}
