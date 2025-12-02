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
use std::path::{Path, PathBuf};

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

    /// Optional glob pattern to filter which files this rule applies to
    /// If None, applies to all files
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applies_to: Option<String>,
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
            applies_to: None,
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

        // Validate applies_to glob pattern if present
        if let Some(ref pattern) = self.applies_to {
            if let Err(e) = glob::Pattern::new(pattern) {
                return Err(SwissArmyHammerError::Other {
                    message: format!("Invalid glob pattern in applies_to field: {}", e),
                });
            }
        }

        Ok(())
    }

    /// Get allowed tools regex patterns from metadata
    ///
    /// Returns the list of regex patterns for tools that are allowed during rule checking.
    /// If not specified in metadata, returns None.
    pub fn get_allowed_tools_regex(&self) -> Option<Vec<String>> {
        self.metadata
            .get("allowed_tools_regex")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
    }

    /// Get denied tools regex patterns from metadata
    ///
    /// Returns the list of regex patterns for tools that are denied during rule checking.
    /// If not specified in metadata, returns None.
    pub fn get_denied_tools_regex(&self) -> Option<Vec<String>> {
        self.metadata
            .get("denied_tools_regex")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
    }

    /// Check if this rule has tool filtering configuration
    ///
    /// Returns true if either allowed_tools_regex or denied_tools_regex is specified in metadata.
    pub fn has_tool_filter(&self) -> bool {
        self.get_allowed_tools_regex().is_some() || self.get_denied_tools_regex().is_some()
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
            applies_to: None,
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
    applies_to: Option<String>,
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

    /// Set applies_to glob pattern
    pub fn applies_to(mut self, pattern: String) -> Self {
        self.applies_to = Some(pattern);
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
            applies_to: self.applies_to,
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

/// Manages a collection of rules with storage and retrieval capabilities
///
/// RuleLibrary provides methods to load rules from directories, search through them,
/// and manage them programmatically. Uses a pluggable storage backend system.
///
/// # Examples
///
/// ```
/// use swissarmyhammer_rules::RuleLibrary;
///
/// // Create a new library with in-memory storage
/// let mut library = RuleLibrary::new();
///
/// // Add rules from a directory
/// let count = library.add_directory("./.swissarmyhammer/rules").unwrap();
/// println!("Loaded {} rules", count);
///
/// // Get a specific rule
/// let rule = library.get("no-hardcoded-secrets").unwrap();
/// ```
pub struct RuleLibrary {
    storage: Box<dyn crate::StorageBackend>,
}

impl std::fmt::Debug for RuleLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuleLibrary")
            .field("storage", &"<StorageBackend>")
            .finish()
    }
}

impl RuleLibrary {
    /// Creates a new rule library with default in-memory storage
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer_rules::RuleLibrary;
    ///
    /// let library = RuleLibrary::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            storage: Box::new(crate::storage::MemoryStorage::new()),
        }
    }

    /// Creates a rule library with a custom storage backend
    ///
    /// # Arguments
    ///
    /// * `storage` - The storage backend to use
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer_rules::{RuleLibrary, MemoryStorage};
    ///
    /// let storage = Box::new(MemoryStorage::new());
    /// let library = RuleLibrary::with_storage(storage);
    /// ```
    #[must_use]
    pub fn with_storage(storage: Box<dyn crate::StorageBackend>) -> Self {
        Self { storage }
    }

    /// Loads all rules from a directory and adds them to the library
    ///
    /// Recursively scans the directory for markdown files with rule definitions.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the directory containing rule files
    ///
    /// # Returns
    ///
    /// The number of rules successfully loaded
    ///
    /// # Errors
    ///
    /// Returns an error if directory does not exist or storage fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use swissarmyhammer_rules::RuleLibrary;
    ///
    /// let mut library = RuleLibrary::new();
    /// let count = library.add_directory("./.swissarmyhammer/rules").unwrap();
    /// println!("Loaded {} rules", count);
    /// ```
    pub fn add_directory(&mut self, path: impl AsRef<Path>) -> Result<usize> {
        let loader = crate::RuleLoader::new();
        let rules = loader.load_directory(path)?;
        let count = rules.len();

        for rule in rules {
            self.storage.store(&rule.name, &rule)?;
        }

        Ok(count)
    }

    /// Adds a single rule to the library
    ///
    /// # Arguments
    ///
    /// * `rule` - The rule to add
    ///
    /// # Errors
    ///
    /// Returns an error if storage backend fails
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer_rules::{RuleLibrary, Rule, Severity};
    ///
    /// let mut library = RuleLibrary::new();
    /// let rule = Rule::new(
    ///     "test-rule".to_string(),
    ///     "Check something".to_string(),
    ///     Severity::Error,
    /// );
    /// library.add(rule).unwrap();
    /// ```
    pub fn add(&mut self, rule: Rule) -> Result<()> {
        self.storage.store(&rule.name, &rule)
    }

    /// Retrieves a rule by its name
    ///
    /// # Arguments
    ///
    /// * `name` - The unique name of the rule
    ///
    /// # Returns
    ///
    /// The rule if found
    ///
    /// # Errors
    ///
    /// Returns an error if rule not found or storage fails
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer_rules::{RuleLibrary, Rule, Severity};
    ///
    /// let mut library = RuleLibrary::new();
    /// let rule = Rule::new("test".to_string(), "Template".to_string(), Severity::Error);
    /// library.add(rule).unwrap();
    ///
    /// let retrieved = library.get("test").unwrap();
    /// assert_eq!(retrieved.name, "test");
    /// ```
    pub fn get(&self, name: &str) -> Result<Rule> {
        self.storage
            .get(name)?
            .ok_or_else(|| SwissArmyHammerError::Other {
                message: format!("Rule '{}' not found", name),
            })
    }

    /// Lists all rules in the library
    ///
    /// # Returns
    ///
    /// A vector of all rules in the library
    ///
    /// # Errors
    ///
    /// Returns an error if storage backend fails
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer_rules::{RuleLibrary, Rule, Severity};
    ///
    /// let mut library = RuleLibrary::new();
    /// library.add(Rule::new("test1".to_string(), "T1".to_string(), Severity::Error)).unwrap();
    /// library.add(Rule::new("test2".to_string(), "T2".to_string(), Severity::Warning)).unwrap();
    ///
    /// let rules = library.list().unwrap();
    /// assert_eq!(rules.len(), 2);
    /// ```
    pub fn list(&self) -> Result<Vec<Rule>> {
        self.storage.list()
    }

    /// Lists all rule names in the library
    ///
    /// # Returns
    ///
    /// A vector of all rule names
    ///
    /// # Errors
    ///
    /// Returns an error if storage backend fails
    pub fn list_names(&self) -> Result<Vec<String>> {
        self.storage.list_keys()
    }

    /// Removes a rule from the library
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the rule to remove
    ///
    /// # Returns
    ///
    /// `true` if the rule was removed, `false` if it didn't exist
    ///
    /// # Errors
    ///
    /// Returns an error if storage backend fails
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer_rules::{RuleLibrary, Rule, Severity};
    ///
    /// let mut library = RuleLibrary::new();
    /// let rule = Rule::new("test".to_string(), "T".to_string(), Severity::Error);
    /// library.add(rule).unwrap();
    ///
    /// assert!(library.remove("test").unwrap());
    /// assert!(!library.remove("test").unwrap());
    /// ```
    pub fn remove(&mut self, name: &str) -> Result<bool> {
        self.storage.remove(name)
    }

    /// Searches for rules by name pattern
    ///
    /// # Arguments
    ///
    /// * `pattern` - The pattern to match against rule names
    ///
    /// # Returns
    ///
    /// A vector of matching rules
    ///
    /// # Errors
    ///
    /// Returns an error if storage backend fails
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer_rules::{RuleLibrary, Rule, Severity};
    ///
    /// let mut library = RuleLibrary::new();
    /// library.add(Rule::new("no-secrets".to_string(), "T".to_string(), Severity::Error)).unwrap();
    /// library.add(Rule::new("no-eval".to_string(), "T".to_string(), Severity::Error)).unwrap();
    ///
    /// let results = library.search("no-").unwrap();
    /// assert_eq!(results.len(), 2);
    /// ```
    pub fn search(&self, pattern: &str) -> Result<Vec<Rule>> {
        let all_rules = self.list()?;
        let filtered = all_rules
            .into_iter()
            .filter(|rule| rule.name.contains(pattern))
            .collect();
        Ok(filtered)
    }

    /// Lists rules with filtering applied
    ///
    /// # Arguments
    ///
    /// * `filter` - The filter criteria to apply
    /// * `sources` - Map of rule names to their sources
    ///
    /// # Returns
    ///
    /// A vector of filtered rules
    ///
    /// # Errors
    ///
    /// Returns an error if storage backend fails
    pub fn list_filtered(
        &self,
        filter: &crate::RuleFilter,
        sources: &HashMap<String, crate::RuleSource>,
    ) -> Result<Vec<Rule>> {
        let all_rules = self.list()?;
        let rule_refs: Vec<&Rule> = all_rules.iter().collect();
        Ok(filter.apply(rule_refs, sources))
    }
}

impl Default for RuleLibrary {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod library_tests {
    use super::*;

    #[test]
    fn test_rule_library_new() {
        let library = RuleLibrary::new();
        let rules = library.list().unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn test_rule_library_add_and_get() {
        let mut library = RuleLibrary::new();
        let rule = Rule::new(
            "test-rule".to_string(),
            "Check something".to_string(),
            Severity::Error,
        );

        library.add(rule.clone()).unwrap();

        let retrieved = library.get("test-rule").unwrap();
        assert_eq!(retrieved.name, "test-rule");
        assert_eq!(retrieved.severity, Severity::Error);
    }

    #[test]
    fn test_rule_library_list() {
        let mut library = RuleLibrary::new();

        library
            .add(Rule::new(
                "rule1".to_string(),
                "Template 1".to_string(),
                Severity::Error,
            ))
            .unwrap();
        library
            .add(Rule::new(
                "rule2".to_string(),
                "Template 2".to_string(),
                Severity::Warning,
            ))
            .unwrap();

        let rules = library.list().unwrap();
        assert_eq!(rules.len(), 2);
    }

    #[test]
    fn test_rule_library_list_names() {
        let mut library = RuleLibrary::new();

        library
            .add(Rule::new(
                "rule1".to_string(),
                "T1".to_string(),
                Severity::Error,
            ))
            .unwrap();
        library
            .add(Rule::new(
                "rule2".to_string(),
                "T2".to_string(),
                Severity::Error,
            ))
            .unwrap();

        let names = library.list_names().unwrap();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"rule1".to_string()));
        assert!(names.contains(&"rule2".to_string()));
    }

    #[test]
    fn test_rule_library_remove() {
        let mut library = RuleLibrary::new();
        let rule = Rule::new("test".to_string(), "T".to_string(), Severity::Error);

        library.add(rule).unwrap();
        assert!(library.get("test").is_ok());

        assert!(library.remove("test").unwrap());
        assert!(library.get("test").is_err());

        assert!(!library.remove("test").unwrap());
    }

    #[test]
    fn test_rule_library_search() {
        let mut library = RuleLibrary::new();

        library
            .add(Rule::new(
                "no-secrets".to_string(),
                "T".to_string(),
                Severity::Error,
            ))
            .unwrap();
        library
            .add(Rule::new(
                "no-eval".to_string(),
                "T".to_string(),
                Severity::Error,
            ))
            .unwrap();
        library
            .add(Rule::new(
                "function-length".to_string(),
                "T".to_string(),
                Severity::Warning,
            ))
            .unwrap();

        let results = library.search("no-").unwrap();
        assert_eq!(results.len(), 2);

        let results = library.search("function").unwrap();
        assert_eq!(results.len(), 1);

        let results = library.search("nonexistent").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_rule_library_get_not_found() {
        let library = RuleLibrary::new();
        let result = library.get("nonexistent");
        assert!(result.is_err());
    }
}
