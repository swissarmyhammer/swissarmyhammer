//! Rule loading functionality
//!
//! This module provides functionality to load rules from files and directories,
//! parsing frontmatter and creating Rule instances.

use crate::{Result, Rule, Severity};
use std::path::Path;
use swissarmyhammer_common::SwissArmyHammerError;
use walkdir::WalkDir;

/// Loads rules from various sources
///
/// The `RuleLoader` is responsible for discovering and parsing rule files
/// from directories, handling various file extensions and formats.
///
/// # Examples
///
/// ```no_run
/// use swissarmyhammer_rules::RuleLoader;
///
/// let loader = RuleLoader::new();
/// let rules = loader.load_directory("./rules").unwrap();
/// println!("Loaded {} rules", rules.len());
/// ```
pub struct RuleLoader {
    /// File extensions to consider as rule files
    extensions: Vec<String>,
}

impl RuleLoader {
    /// Create a new rule loader with default extensions
    ///
    /// Supports the following extensions:
    /// - `.md`
    /// - `.md.liquid`
    /// - `.liquid.md`
    /// - `.markdown`
    /// - `.markdown.liquid`
    /// - `.liquid.markdown`
    /// - `.liquid`
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer_rules::RuleLoader;
    ///
    /// let loader = RuleLoader::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            extensions: vec![
                "md".to_string(),
                "md.liquid".to_string(),
                "liquid.md".to_string(),
                "markdown".to_string(),
                "markdown.liquid".to_string(),
                "liquid.markdown".to_string(),
                "liquid".to_string(),
            ],
        }
    }

    /// Load all rules from a directory recursively
    ///
    /// Scans the directory and all subdirectories for files matching
    /// the configured extensions, parsing each as a rule.
    ///
    /// # Arguments
    ///
    /// * `path` - Directory path to scan
    ///
    /// # Returns
    ///
    /// A vector of successfully loaded rules
    ///
    /// # Errors
    ///
    /// Returns an error if the directory does not exist or cannot be read
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use swissarmyhammer_rules::RuleLoader;
    ///
    /// let loader = RuleLoader::new();
    /// let rules = loader.load_directory("./rules").unwrap();
    /// for rule in rules {
    ///     println!("Loaded rule: {}", rule.name);
    /// }
    /// ```
    pub fn load_directory(&self, path: impl AsRef<Path>) -> Result<Vec<Rule>> {
        let path = path.as_ref();
        let mut rules = Vec::new();

        if !path.exists() {
            return Err(SwissArmyHammerError::FileNotFound {
                path: path.display().to_string(),
                suggestion: "Check that the directory path is correct and accessible".to_string(),
            });
        }

        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            let entry_path = entry.path();
            if entry_path.is_file() && self.is_rule_file(entry_path) {
                if let Ok(rule) = self.load_file_with_base(entry_path, path) {
                    rules.push(rule);
                }
            }
        }

        // Sort rules by name for consistent ordering
        rules.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(rules)
    }

    /// Load a single rule from a file
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the rule file
    ///
    /// # Returns
    ///
    /// The loaded rule
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use swissarmyhammer_rules::RuleLoader;
    ///
    /// let loader = RuleLoader::new();
    /// let rule = loader.load_file("./rules/my-rule.md").unwrap();
    /// println!("Loaded: {}", rule.name);
    /// ```
    pub fn load_file(&self, path: impl AsRef<Path>) -> Result<Rule> {
        let path_ref = path.as_ref();
        self.load_file_with_base(path_ref, path_ref.parent().unwrap_or(path_ref))
    }

    /// Load a rule from a string
    ///
    /// # Arguments
    ///
    /// * `name` - Name for the rule
    /// * `content` - String content with optional frontmatter
    ///
    /// # Returns
    ///
    /// The loaded rule
    ///
    /// # Errors
    ///
    /// Returns an error if the content cannot be parsed
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer_rules::RuleLoader;
    ///
    /// let content = r#"---
    /// title: Test Rule
    /// description: A test rule
    /// severity: error
    /// ---
    ///
    /// Check for issues
    /// "#;
    ///
    /// let loader = RuleLoader::new();
    /// let rule = loader.load_from_string("test", content).unwrap();
    /// assert_eq!(rule.name, "test");
    /// ```
    pub fn load_from_string(&self, name: &str, content: &str) -> Result<Rule> {
        let (metadata, template) = Self::parse_front_matter(content)?;

        // Check if this is a partial template
        let has_partial_marker = content.trim_start().starts_with("{% partial %}");

        let mut rule = if let Some(ref metadata_value) = metadata {
            // Parse severity from metadata
            // Default to Warning when frontmatter exists but severity is not specified
            // This assumes the author intentionally added frontmatter and would have
            // specified Error if they wanted it, so Warning is a safer default
            let severity = metadata_value
                .get("severity")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<Severity>().ok())
                .unwrap_or(Severity::Warning);

            Rule::new(name.to_string(), template.clone(), severity)
        } else {
            // No frontmatter at all - default to Error severity
            // Rules without frontmatter are considered more critical and should
            // fail loudly to ensure they are properly configured with metadata
            Rule::new(name.to_string(), template.clone(), Severity::Error)
        };

        // Parse metadata fields
        if let Some(ref metadata_value) = metadata {
            if let Some(title) = metadata_value.get("title").and_then(|v| v.as_str()) {
                rule.metadata.insert(
                    "title".to_string(),
                    serde_json::Value::String(title.to_string()),
                );
            }

            if let Some(desc) = metadata_value.get("description").and_then(|v| v.as_str()) {
                rule.description = Some(desc.to_string());
            }

            if let Some(cat) = metadata_value.get("category").and_then(|v| v.as_str()) {
                rule.category = Some(cat.to_string());
            }

            if let Some(tags) = metadata_value.get("tags").and_then(|v| v.as_array()) {
                rule.tags = tags
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect();
            }

            if let Some(auto_fix) = metadata_value.get("auto_fix").and_then(|v| v.as_bool()) {
                rule.auto_fix = auto_fix;
            }

            if let Some(applies_to) = metadata_value.get("applies_to").and_then(|v| v.as_str()) {
                rule.applies_to = Some(applies_to.to_string());
            }
        }

        // Set default description for partials
        if rule.description.is_none()
            && (has_partial_marker || Self::is_likely_partial(name, content))
        {
            rule.description = Some("Partial template for reuse in other rules".to_string());
        }

        Ok(rule)
    }

    /// Load a rule file with base path for relative naming
    fn load_file_with_base(&self, path: &Path, base_path: &Path) -> Result<Rule> {
        let content = std::fs::read_to_string(path)?;
        let name = self.extract_rule_name_with_base(path, base_path);

        let mut rule = self.load_from_string(&name, &content)?;
        rule.source = Some(path.to_path_buf());

        Ok(rule)
    }

    /// Check if a path is a rule file based on extension
    fn is_rule_file(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();
        self.extensions
            .iter()
            .any(|ext| path_str.ends_with(&format!(".{ext}")))
    }

    /// Extract rule name from file path, handling compound extensions
    fn extract_rule_name(&self, path: &Path) -> String {
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();

        // Sort extensions by length descending to match longest first
        let mut sorted_extensions = self.extensions.clone();
        sorted_extensions.sort_by_key(|b| std::cmp::Reverse(b.len()));

        // Remove supported extensions, checking longest first
        for ext in &sorted_extensions {
            let extension = format!(".{ext}");
            if filename.ends_with(&extension) {
                return filename[..filename.len() - extension.len()].to_string();
            }
        }

        // Fallback to file_stem behavior
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string()
    }

    /// Extract rule name with relative path from base directory
    fn extract_rule_name_with_base(&self, path: &Path, base_path: &Path) -> String {
        // Get relative path from base
        let relative_path = path.strip_prefix(base_path).unwrap_or(path);

        // Get the path without the filename
        let mut name_path = String::new();
        if let Some(parent) = relative_path.parent() {
            if parent != Path::new("") {
                name_path = parent.to_string_lossy().replace('\\', "/");
                name_path.push('/');
            }
        }

        // Extract filename without extension
        let filename = self.extract_rule_name(path);
        name_path.push_str(&filename);

        name_path
    }

    /// Parse front matter from content
    fn parse_front_matter(content: &str) -> Result<(Option<serde_json::Value>, String)> {
        let frontmatter = crate::frontmatter::parse_frontmatter(content)?;
        Ok((frontmatter.metadata, frontmatter.content))
    }

    /// Determine if a rule is likely a partial template
    fn is_likely_partial(name: &str, content: &str) -> bool {
        // Check if the name suggests it's a partial
        let name_lower = name.to_lowercase();
        if name_lower.contains("partial") || name_lower.starts_with('_') {
            return true;
        }

        // Check if it has no YAML front matter
        let has_front_matter = content.starts_with("---\n");
        if !has_front_matter {
            return true;
        }

        // Check for typical partial characteristics
        let lines: Vec<&str> = content.lines().collect();
        let content_lines: Vec<&str> = if has_front_matter {
            // Skip YAML front matter
            lines
                .iter()
                .skip_while(|line| **line != "---")
                .skip(1)
                .skip_while(|line| **line != "---")
                .skip(1)
                .copied()
                .collect()
        } else {
            lines
        };

        // If it's very short and has no headers, it might be a partial
        if content_lines.len() <= 5 && !content_lines.iter().any(|line| line.starts_with('#')) {
            return true;
        }

        false
    }
}

impl Default for RuleLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_loader_new() {
        let loader = RuleLoader::new();
        assert!(!loader.extensions.is_empty());
        assert!(loader.extensions.contains(&"md".to_string()));
        assert!(loader.extensions.contains(&"md.liquid".to_string()));
    }

    #[test]
    fn test_extension_stripping() {
        let loader = RuleLoader::new();

        let test_cases = vec![
            ("test.md", "test"),
            ("test.liquid.md", "test"),
            ("test.md.liquid", "test"),
            ("test.liquid", "test"),
            ("partials/header.liquid.md", "header"),
        ];

        for (filename, expected) in test_cases {
            let path = Path::new(filename);
            let result = loader.extract_rule_name(path);
            assert_eq!(result, expected, "Failed for {filename}");
        }
    }

    #[test]
    fn test_is_rule_file() {
        let loader = RuleLoader::new();

        assert!(loader.is_rule_file(Path::new("test.md")));
        assert!(loader.is_rule_file(Path::new("test.md.liquid")));
        assert!(loader.is_rule_file(Path::new("test.liquid.md")));
        assert!(loader.is_rule_file(Path::new("test.markdown")));
        assert!(!loader.is_rule_file(Path::new("test.txt")));
        assert!(!loader.is_rule_file(Path::new("test.rs")));
    }

    #[test]
    fn test_load_from_string_basic() {
        let loader = RuleLoader::new();
        let content = r#"---
title: Test Rule
description: A test rule
severity: error
category: testing
tags: ["test", "example"]
---

Check for test issues
"#;

        let rule = loader.load_from_string("test-rule", content).unwrap();
        assert_eq!(rule.name, "test-rule");
        assert_eq!(rule.severity, Severity::Error);
        assert_eq!(rule.description, Some("A test rule".to_string()));
        assert_eq!(rule.category, Some("testing".to_string()));
        assert_eq!(rule.tags, vec!["test", "example"]);
        assert_eq!(rule.template.trim(), "Check for test issues");
    }

    #[test]
    fn test_load_from_string_partial() {
        let loader = RuleLoader::new();
        let content = "{% partial %}\n\nCommon checking patterns";

        let rule = loader.load_from_string("_partial", content).unwrap();
        assert!(rule.is_partial());
        assert_eq!(
            rule.description,
            Some("Partial template for reuse in other rules".to_string())
        );
    }

    #[test]
    fn test_load_from_string_no_metadata() {
        let loader = RuleLoader::new();
        let content = "Just some content without frontmatter";

        let rule = loader.load_from_string("simple", content).unwrap();
        assert_eq!(rule.name, "simple");
        assert_eq!(rule.severity, Severity::Error); // Default for no metadata
        assert_eq!(rule.template, "Just some content without frontmatter");
    }

    #[test]
    fn test_load_from_string_with_auto_fix() {
        let loader = RuleLoader::new();
        let content = r#"---
title: Auto Fix Rule
description: Can auto-fix
severity: warning
auto_fix: true
---

Fix this automatically
"#;

        let rule = loader.load_from_string("auto-fix", content).unwrap();
        assert!(rule.auto_fix);
        assert_eq!(rule.severity, Severity::Warning);
    }

    #[test]
    fn test_is_likely_partial() {
        assert!(RuleLoader::is_likely_partial("_header", "content"));
        assert!(RuleLoader::is_likely_partial("common-partial", "content"));
        assert!(RuleLoader::is_likely_partial("regular", "short\ncontent"));
        assert!(!RuleLoader::is_likely_partial(
            "regular",
            "---\ntitle: Test\n---\n# Header\nLong content here"
        ));
    }

    #[test]
    fn test_extract_rule_name_with_base() {
        let loader = RuleLoader::new();
        let base = Path::new("/rules");
        let path = Path::new("/rules/security/no-secrets.md");

        let name = loader.extract_rule_name_with_base(path, base);
        assert_eq!(name, "security/no-secrets");
    }

    #[test]
    fn test_load_directory_not_found() {
        let loader = RuleLoader::new();
        let result = loader.load_directory("/nonexistent/path");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_from_string_frontmatter_without_severity() {
        let loader = RuleLoader::new();
        let content = r#"---
title: Test Rule Without Severity
description: Rule with frontmatter but no severity field
---

Check something
"#;

        let rule = loader.load_from_string("test", content).unwrap();
        assert_eq!(rule.name, "test");
        assert_eq!(rule.severity, Severity::Warning); // Default when frontmatter exists but no severity
        assert_eq!(
            rule.description,
            Some("Rule with frontmatter but no severity field".to_string())
        );
    }

    #[test]
    fn test_load_from_string_no_frontmatter_defaults_to_error() {
        let loader = RuleLoader::new();
        let content = "Check for security issues in the code";

        let rule = loader.load_from_string("security-check", content).unwrap();
        assert_eq!(rule.name, "security-check");
        assert_eq!(rule.severity, Severity::Error); // No frontmatter defaults to Error
        assert_eq!(rule.template, "Check for security issues in the code");
    }
}
