//! Display objects for rule command output
//!
//! Provides clean display objects with `Tabled` and `Serialize` derives for consistent
//! output formatting across table, JSON, and YAML formats.

use serde::{Deserialize, Serialize};
use tabled::Tabled;

/// Basic rule information for standard list output
#[derive(Tabled, Serialize, Deserialize, Debug, Clone)]
pub struct RuleRow {
    #[tabled(rename = "Name")]
    pub name: String,

    #[tabled(rename = "Title")]
    pub title: String,

    #[tabled(rename = "Severity")]
    pub severity: String,

    #[tabled(rename = "Source")]
    pub source: String,
}

/// Detailed rule information for verbose list output
#[derive(Tabled, Serialize, Deserialize, Debug, Clone)]
pub struct VerboseRuleRow {
    #[tabled(rename = "Name")]
    pub name: String,

    #[tabled(rename = "Title")]
    pub title: String,

    #[tabled(rename = "Description")]
    pub description: String,

    #[tabled(rename = "Severity")]
    pub severity: String,

    #[tabled(rename = "Category")]
    pub category: String,

    #[tabled(rename = "Source")]
    pub source: String,
}

impl RuleRow {
    /// Create a RuleRow with FileSource information for emoji-based source display
    pub fn from_rule_with_source(
        rule: &swissarmyhammer_rules::Rule,
        file_source: Option<&swissarmyhammer::FileSource>,
    ) -> Self {
        // Try to get title from metadata, fall back to name
        let title = rule
            .metadata
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| rule.name.clone());

        Self {
            name: rule.name.clone(),
            title,
            severity: format!("{:?}", rule.severity).to_lowercase(),
            source: file_source
                .map(|s| s.display_emoji())
                .unwrap_or("📦 Built-in")
                .to_string(),
        }
    }
}

impl VerboseRuleRow {
    /// Create a VerboseRuleRow with FileSource information for emoji-based source display
    pub fn from_rule_with_source(
        rule: &swissarmyhammer_rules::Rule,
        file_source: Option<&swissarmyhammer::FileSource>,
    ) -> Self {
        // Try to get title from metadata, fall back to name
        let title = rule
            .metadata
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| rule.name.clone());

        Self {
            name: rule.name.clone(),
            title,
            description: rule
                .description
                .clone()
                .unwrap_or_else(|| "No description".to_string()),
            severity: format!("{:?}", rule.severity).to_lowercase(),
            category: rule.category.clone().unwrap_or_default(),
            source: file_source
                .map(|s| s.display_emoji())
                .unwrap_or("📦 Built-in")
                .to_string(),
        }
    }
}

/// Convert rules with source information to appropriate display format with emoji-based sources
pub fn rules_to_display_rows_with_sources(
    rules: Vec<swissarmyhammer_rules::Rule>,
    sources: &std::collections::HashMap<String, swissarmyhammer::FileSource>,
    verbose: bool,
) -> DisplayRows {
    if verbose {
        DisplayRows::Verbose(
            rules
                .iter()
                .map(|rule| {
                    let file_source = sources.get(&rule.name);
                    VerboseRuleRow::from_rule_with_source(rule, file_source)
                })
                .collect(),
        )
    } else {
        DisplayRows::Standard(
            rules
                .iter()
                .map(|rule| {
                    let file_source = sources.get(&rule.name);
                    RuleRow::from_rule_with_source(rule, file_source)
                })
                .collect(),
        )
    }
}

/// Enum to handle different display row types
#[derive(Debug)]
pub enum DisplayRows {
    Standard(Vec<RuleRow>),
    Verbose(Vec<VerboseRuleRow>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use swissarmyhammer_rules::Rule;

    fn create_test_rule() -> Rule {
        let mut metadata = HashMap::new();
        metadata.insert("title".to_string(), serde_json::json!("Test Rule"));

        Rule {
            name: "test-rule".to_string(),
            template: "Test template".to_string(),
            description: Some("Test description".to_string()),
            category: Some("rust".to_string()),
            tags: vec![],
            source: Some(std::path::PathBuf::from("/test/path/test-rule.md")),
            metadata,
            severity: swissarmyhammer_rules::Severity::Error,
            auto_fix: false,
        }
    }

    fn create_empty_rule() -> Rule {
        Rule {
            name: "empty-rule".to_string(),
            template: String::new(),
            description: None,
            category: None,
            tags: vec![],
            source: None,
            metadata: HashMap::new(),
            severity: swissarmyhammer_rules::Severity::Error,
            auto_fix: false,
        }
    }

    #[test]
    fn test_rule_row_with_source_emoji_mapping() {
        let rule = create_test_rule();

        let builtin_row =
            RuleRow::from_rule_with_source(&rule, Some(&swissarmyhammer::FileSource::Builtin));
        assert_eq!(builtin_row.source, "📦 Built-in");
        assert_eq!(builtin_row.severity, "error");

        let project_row =
            RuleRow::from_rule_with_source(&rule, Some(&swissarmyhammer::FileSource::Local));
        assert_eq!(project_row.source, "📁 Project");
        assert_eq!(project_row.severity, "error");

        let user_row =
            RuleRow::from_rule_with_source(&rule, Some(&swissarmyhammer::FileSource::User));
        assert_eq!(user_row.source, "👤 User");
        assert_eq!(user_row.severity, "error");

        let dynamic_row =
            RuleRow::from_rule_with_source(&rule, Some(&swissarmyhammer::FileSource::Dynamic));
        assert_eq!(dynamic_row.source, "📦 Built-in");
        assert_eq!(dynamic_row.severity, "error");

        let no_source_row = RuleRow::from_rule_with_source(&rule, None);
        assert_eq!(no_source_row.source, "📦 Built-in");
        assert_eq!(no_source_row.severity, "error");
    }

    #[test]
    fn test_rule_row_from_empty_rule() {
        let rule = create_empty_rule();
        let row = RuleRow::from_rule_with_source(&rule, None);
        assert_eq!(row.name, "empty-rule");
        assert_eq!(row.title, "empty-rule"); // Falls back to name when no title in metadata
        assert_eq!(row.severity, "error");
        assert_eq!(row.source, "📦 Built-in");
    }

    #[test]
    fn test_verbose_rule_row_conversion() {
        let rule = create_test_rule();
        let row = VerboseRuleRow::from_rule_with_source(&rule, None);
        assert_eq!(row.name, "test-rule");
        assert_eq!(row.title, "Test Rule");
        assert_eq!(row.description, "Test description");
        assert_eq!(row.severity, "error");
        assert_eq!(row.category, "rust"); // Comes from category
        assert_eq!(row.source, "📦 Built-in");
    }

    #[test]
    fn test_verbose_rule_row_from_empty_rule() {
        let rule = create_empty_rule();
        let row = VerboseRuleRow::from_rule_with_source(&rule, None);
        assert_eq!(row.name, "empty-rule");
        assert_eq!(row.title, "empty-rule"); // Falls back to name when no title in metadata
        assert_eq!(row.description, "No description");
        assert_eq!(row.severity, "error");
        assert_eq!(row.category, "");
        assert_eq!(row.source, "📦 Built-in");
    }

    #[test]
    fn test_rules_to_display_rows_standard() {
        let rules = vec![create_test_rule()];
        let sources = HashMap::new();
        let rows = rules_to_display_rows_with_sources(rules, &sources, false);

        match rows {
            DisplayRows::Standard(standard_rows) => {
                assert_eq!(standard_rows.len(), 1);
                assert_eq!(standard_rows[0].name, "test-rule");
            }
            DisplayRows::Verbose(_) => panic!("Expected Standard rows"),
        }
    }

    #[test]
    fn test_rules_to_display_rows_verbose() {
        let rules = vec![create_test_rule()];
        let sources = HashMap::new();
        let rows = rules_to_display_rows_with_sources(rules, &sources, true);

        match rows {
            DisplayRows::Verbose(verbose_rows) => {
                assert_eq!(verbose_rows.len(), 1);
                assert_eq!(verbose_rows[0].name, "test-rule");
                assert_eq!(verbose_rows[0].description, "Test description");
            }
            DisplayRows::Standard(_) => panic!("Expected Verbose rows"),
        }
    }

    #[test]
    fn test_rules_to_display_rows_with_sources_emoji_mapping() {
        let rules = vec![create_test_rule()];
        let mut sources = HashMap::new();
        sources.insert("test-rule".to_string(), swissarmyhammer::FileSource::Local);

        let standard_rows = rules_to_display_rows_with_sources(rules.clone(), &sources, false);
        match standard_rows {
            DisplayRows::Standard(rows) => {
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].name, "test-rule");
                assert_eq!(rows[0].source, "📁 Project");
            }
            _ => panic!("Expected Standard rows"),
        }

        let verbose_rows = rules_to_display_rows_with_sources(rules, &sources, true);
        match verbose_rows {
            DisplayRows::Verbose(rows) => {
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].name, "test-rule");
                assert_eq!(rows[0].source, "📁 Project");
            }
            _ => panic!("Expected Verbose rows"),
        }
    }

    #[test]
    fn test_serialization_rule_row() {
        let row = RuleRow {
            name: "test".to_string(),
            title: "Test Title".to_string(),
            severity: "error".to_string(),
            source: "Test Source".to_string(),
        };

        let json = serde_json::to_string(&row).expect("Should serialize to JSON");
        assert!(json.contains("test"));
        assert!(json.contains("Test Title"));
        assert!(json.contains("error"));
        assert!(json.contains("Test Source"));

        let deserialized: RuleRow =
            serde_json::from_str(&json).expect("Should deserialize from JSON");
        assert_eq!(deserialized.name, "test");
        assert_eq!(deserialized.title, "Test Title");
        assert_eq!(deserialized.severity, "error");
        assert_eq!(deserialized.source, "Test Source");
    }

    #[test]
    fn test_serialization_verbose_rule_row() {
        let row = VerboseRuleRow {
            name: "test".to_string(),
            title: "Test Title".to_string(),
            description: "Test Description".to_string(),
            severity: "error".to_string(),
            category: "Rust".to_string(),
            source: "Test Source".to_string(),
        };

        let json = serde_json::to_string(&row).expect("Should serialize to JSON");
        assert!(json.contains("test"));
        assert!(json.contains("Test Title"));
        assert!(json.contains("Test Description"));
        assert!(json.contains("error"));
        assert!(json.contains("Rust"));

        let deserialized: VerboseRuleRow =
            serde_json::from_str(&json).expect("Should deserialize from JSON");
        assert_eq!(deserialized.name, "test");
        assert_eq!(deserialized.title, "Test Title");
        assert_eq!(deserialized.description, "Test Description");
        assert_eq!(deserialized.severity, "error");
        assert_eq!(deserialized.category, "Rust");
        assert_eq!(deserialized.source, "Test Source");
    }

    #[test]
    fn test_display_rows_debug_format() {
        let rules = vec![create_test_rule()];
        let sources = HashMap::new();
        let rows = rules_to_display_rows_with_sources(rules, &sources, false);

        let debug_str = format!("{:?}", rows);
        assert!(debug_str.contains("Standard"));
        assert!(debug_str.contains("test-rule"));
    }
}
