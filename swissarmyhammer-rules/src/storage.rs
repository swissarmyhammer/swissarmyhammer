//! Storage backend trait and implementations for rule libraries
//!
//! This module defines the storage abstraction used by rule libraries
//! to persist and retrieve rules from various storage backends.
//!
//! # Storage Backends
//!
//! Two storage implementations are provided:
//!
//! ## MemoryStorage
//!
//! In-memory storage using a HashMap. Rules are lost when the application exits.
//! Useful for testing and temporary rule sets.
//!
//! ## FileStorage
//!
//! File-based storage that persists rules as markdown files with YAML frontmatter.
//! Each rule is stored in a separate `.md` file under a base directory.
//!
//! ### File Format Example
//!
//! ```markdown
//! ---
//! name: no-hardcoded-secrets
//! description: Check for hardcoded API keys and tokens
//! category: security
//! tags:
//!   - security
//!   - critical
//! severity: Error
//! auto_fix: false
//! ---
//! Check the code for hardcoded secrets like {{secret_type}}.
//! ```
//!
//! # Usage Example
//!
//! ```rust
//! use swissarmyhammer_rules::{Rule, Severity, StorageBackend, FileStorage};
//! use std::path::PathBuf;
//!
//! # fn example() -> swissarmyhammer_rules::Result<()> {
//! // Create file storage
//! let mut storage = FileStorage::new(PathBuf::from("/path/to/rules"));
//!
//! // Create and store a rule
//! let rule = Rule::builder(
//!     "test-rule".to_string(),
//!     "Check for issues".to_string(),
//!     Severity::Error,
//! )
//! .description("Test rule".to_string())
//! .tag("testing".to_string())
//! .build();
//!
//! storage.store("test-rule", &rule)?;
//!
//! // Retrieve the rule
//! let retrieved = storage.get("test-rule")?;
//! # Ok(())
//! # }
//! ```

use crate::rules::Rule;
use crate::Result;
use std::collections::HashMap;
use std::path::Path;
use swissarmyhammer_common::SwissArmyHammerError;

/// Trait for storage backends that can persist and retrieve rules
pub trait StorageBackend: Send + Sync {
    /// Store a rule with the given key
    fn store(&mut self, key: &str, rule: &Rule) -> Result<()>;

    /// Retrieve a rule by key
    fn get(&self, key: &str) -> Result<Option<Rule>>;

    /// List all stored rule keys
    fn list_keys(&self) -> Result<Vec<String>>;

    /// Remove a rule by key
    fn remove(&mut self, key: &str) -> Result<bool>;

    /// Clear all stored rules
    fn clear(&mut self) -> Result<()>;

    /// Check if a rule exists
    fn exists(&self, key: &str) -> Result<bool> {
        Ok(self.get(key)?.is_some())
    }

    /// Get the total number of stored rules
    fn count(&self) -> Result<usize> {
        Ok(self.list_keys()?.len())
    }

    /// List all stored rules
    fn list(&self) -> Result<Vec<Rule>> {
        let keys = self.list_keys()?;
        let mut rules = Vec::new();
        for key in keys {
            if let Some(rule) = self.get(&key)? {
                rules.push(rule);
            }
        }
        Ok(rules)
    }

    /// Search rules by query string
    fn search(&self, query: &str) -> Result<Vec<Rule>> {
        let rules = self.list()?;
        let query_lower = query.to_lowercase();

        Ok(rules
            .into_iter()
            .filter(|rule| {
                rule.name.to_lowercase().contains(&query_lower)
                    || rule
                        .description
                        .as_ref()
                        .map(|d| d.to_lowercase().contains(&query_lower))
                        .unwrap_or(false)
                    || rule.template.to_lowercase().contains(&query_lower)
                    || rule
                        .tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(&query_lower))
                    || format!("{:?}", rule.severity)
                        .to_lowercase()
                        .contains(&query_lower)
            })
            .collect())
    }
}

/// In-memory storage backend for rules
///
/// This is the default storage backend that keeps all rules in memory.
/// Rules are lost when the application exits.
#[derive(Debug, Default)]
pub struct MemoryStorage {
    rules: HashMap<String, Rule>,
}

impl MemoryStorage {
    /// Create a new memory storage backend
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all stored rules
    pub fn get_all(&self) -> &HashMap<String, Rule> {
        &self.rules
    }

    /// Insert a rule directly (for testing)
    pub fn insert(&mut self, key: String, rule: Rule) {
        self.rules.insert(key, rule);
    }
}

impl StorageBackend for MemoryStorage {
    fn store(&mut self, key: &str, rule: &Rule) -> Result<()> {
        self.rules.insert(key.to_string(), rule.clone());
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<Rule>> {
        Ok(self.rules.get(key).cloned())
    }

    fn list_keys(&self) -> Result<Vec<String>> {
        Ok(self.rules.keys().cloned().collect())
    }

    fn remove(&mut self, key: &str) -> Result<bool> {
        Ok(self.rules.remove(key).is_some())
    }

    fn clear(&mut self) -> Result<()> {
        self.rules.clear();
        Ok(())
    }

    fn exists(&self, key: &str) -> Result<bool> {
        Ok(self.rules.contains_key(key))
    }

    fn count(&self) -> Result<usize> {
        Ok(self.rules.len())
    }
}

/// File-based storage backend for rules
///
/// This storage backend persists rules to individual files on disk.
/// Each rule is stored as a separate file with YAML frontmatter + markdown content.
///
/// # File Format
///
/// Rules are stored in markdown files with YAML frontmatter:
///
/// ```markdown
/// ---
/// name: rule-name
/// description: What the rule checks
/// category: security
/// tags:
///   - important
///   - automated
/// severity: Error
/// auto_fix: false
/// metadata:
///   author: username
///   version: "1.0.0"
/// ---
/// # Rule Template Content
///
/// Check for {{pattern}} in the code.
/// ```
///
/// # Required Frontmatter Fields
///
/// - `name`: Rule identifier (string)
/// - `severity`: One of "Error", "Warning", or "Info" (string)
///
/// # Optional Frontmatter Fields
///
/// - `description`: Human-readable description (string)
/// - `category`: Category for organization (string)
/// - `tags`: List of tags for filtering (array of strings)
/// - `auto_fix`: Whether rule supports auto-fixing (boolean, default: false)
/// - `metadata`: Additional key-value pairs (object)
///
/// # File Naming
///
/// Rule keys map directly to filenames with `.md` extension:
/// - Key "no-hardcoded-secrets" → File "no-hardcoded-secrets.md"
/// - Key "security/check-auth" → File "security/check-auth.md" (not currently supported)
///
/// # Directory Structure
///
/// All rule files are stored in a flat structure under the base_path:
/// ```text
/// base_path/
///   ├── rule1.md
///   ├── rule2.md
///   └── rule3.md
/// ```
///
/// Subdirectories are not currently supported for rule organization.
#[derive(Debug)]
pub struct FileStorage {
    base_path: std::path::PathBuf,
}

impl FileStorage {
    /// Create a new file storage backend with the given base directory
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
        }
    }

    /// Get the file path for a given rule key
    fn get_file_path(&self, key: &str) -> std::path::PathBuf {
        self.base_path.join(format!("{}.md", key))
    }
}

impl StorageBackend for FileStorage {
    fn store(&mut self, key: &str, rule: &Rule) -> Result<()> {
        let file_path = self.get_file_path(key);

        // Ensure the directory exists
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| SwissArmyHammerError::Other {
                message: format!("Failed to create directory {}: {}", parent.display(), e),
            })?;
        }

        // Serialize the rule to YAML front matter + content
        let yaml_front_matter = serde_yaml::to_string(&serde_json::json!({
            "name": rule.name,
            "description": rule.description,
            "category": rule.category,
            "tags": rule.tags,
            "severity": rule.severity,
            "auto_fix": rule.auto_fix,
            "metadata": rule.metadata
        }))
        .map_err(|e| SwissArmyHammerError::Other {
            message: format!("Failed to serialize rule metadata: {}", e),
        })?;

        let content = format!("---\n{}---\n{}", yaml_front_matter, rule.template);

        std::fs::write(&file_path, content).map_err(|e| SwissArmyHammerError::Other {
            message: format!("Failed to write rule file {}: {}", file_path.display(), e),
        })?;

        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<Rule>> {
        let file_path = self.get_file_path(key);

        if !file_path.exists() {
            return Ok(None);
        }

        let content =
            std::fs::read_to_string(&file_path).map_err(|e| SwissArmyHammerError::Other {
                message: format!("Failed to read rule file {}: {}", file_path.display(), e),
            })?;

        // Parse the file using the frontmatter parser
        let parsed = crate::parse_frontmatter(&content)?;

        // Extract metadata from frontmatter if present
        let metadata = parsed.metadata.ok_or_else(|| SwissArmyHammerError::Other {
            message: format!("Rule file {} missing frontmatter", file_path.display()),
        })?;

        // Extract required fields
        let name = metadata
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(key)
            .to_string();

        let severity = metadata
            .get("severity")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<crate::Severity>().ok())
            .unwrap_or(crate::Severity::Info);

        // Build the rule using RuleBuilder
        let mut builder = Rule::builder(name, parsed.content, severity);

        // Add optional fields
        if let Some(desc) = metadata.get("description").and_then(|v| v.as_str()) {
            builder = builder.description(desc.to_string());
        }

        if let Some(cat) = metadata.get("category").and_then(|v| v.as_str()) {
            builder = builder.category(cat.to_string());
        }

        if let Some(tags) = metadata.get("tags").and_then(|v| v.as_array()) {
            for tag in tags {
                if let Some(tag_str) = tag.as_str() {
                    builder = builder.tag(tag_str.to_string());
                }
            }
        }

        if let Some(auto_fix) = metadata.get("auto_fix").and_then(|v| v.as_bool()) {
            builder = builder.auto_fix(auto_fix);
        }

        if let Some(meta) = metadata.get("metadata").and_then(|v| v.as_object()) {
            for (k, v) in meta {
                builder = builder.metadata_value(k.clone(), v.clone());
            }
        }

        builder = builder.source(file_path);

        Ok(Some(builder.build()))
    }

    fn list_keys(&self) -> Result<Vec<String>> {
        if !self.base_path.exists() {
            return Ok(Vec::new());
        }

        let entries =
            std::fs::read_dir(&self.base_path).map_err(|e| SwissArmyHammerError::Other {
                message: format!(
                    "Failed to read directory {}: {}",
                    self.base_path.display(),
                    e
                ),
            })?;

        let mut keys = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| SwissArmyHammerError::Other {
                message: format!("Failed to read directory entry: {}", e),
            })?;

            let path = entry.path();
            if path.is_file() && path.extension() == Some(std::ffi::OsStr::new("md")) {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    keys.push(stem.to_string());
                }
            }
        }

        Ok(keys)
    }

    fn remove(&mut self, key: &str) -> Result<bool> {
        let file_path = self.get_file_path(key);

        if !file_path.exists() {
            return Ok(false);
        }

        std::fs::remove_file(&file_path).map_err(|e| SwissArmyHammerError::Other {
            message: format!("Failed to remove rule file {}: {}", file_path.display(), e),
        })?;

        Ok(true)
    }

    fn clear(&mut self) -> Result<()> {
        let keys = self.list_keys()?;
        for key in keys {
            self.remove(&key)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Severity;

    #[test]
    fn test_memory_storage() {
        let mut storage = MemoryStorage::new();
        let rule = Rule::new(
            "test".to_string(),
            "Check for {{pattern}}!".to_string(),
            Severity::Error,
        );

        // Test store and get
        storage.store("test", &rule).unwrap();
        let retrieved = storage.get("test").unwrap().unwrap();
        assert_eq!(retrieved.name, "test");
        assert_eq!(retrieved.template, "Check for {{pattern}}!");
        assert_eq!(retrieved.severity, Severity::Error);

        // Test exists and count
        assert!(storage.exists("test").unwrap());
        assert_eq!(storage.count().unwrap(), 1);

        // Test list_keys
        let keys = storage.list_keys().unwrap();
        assert_eq!(keys, vec!["test"]);

        // Test remove
        assert!(storage.remove("test").unwrap());
        assert!(!storage.exists("test").unwrap());
        assert_eq!(storage.count().unwrap(), 0);
    }

    #[test]
    fn test_memory_storage_clear() {
        let mut storage = MemoryStorage::new();
        let rule1 = Rule::new(
            "test1".to_string(),
            "Template 1".to_string(),
            Severity::Error,
        );
        let rule2 = Rule::new(
            "test2".to_string(),
            "Template 2".to_string(),
            Severity::Warning,
        );

        storage.store("test1", &rule1).unwrap();
        storage.store("test2", &rule2).unwrap();
        assert_eq!(storage.count().unwrap(), 2);

        storage.clear().unwrap();
        assert_eq!(storage.count().unwrap(), 0);
    }

    #[test]
    fn test_memory_storage_search() {
        let mut storage = MemoryStorage::new();

        let rule1 = Rule::builder(
            "security-check".to_string(),
            "Check for security issues".to_string(),
            Severity::Error,
        )
        .description("Security validation".to_string())
        .tag("security".to_string())
        .build();

        let rule2 = Rule::builder(
            "style-check".to_string(),
            "Check code style".to_string(),
            Severity::Warning,
        )
        .description("Style validation".to_string())
        .tag("style".to_string())
        .build();

        storage.store("security-check", &rule1).unwrap();
        storage.store("style-check", &rule2).unwrap();

        // Search by name
        let results = storage.search("security").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "security-check");

        // Search by severity
        let results = storage.search("error").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "security-check");

        // Search by tag
        let results = storage.search("style").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "style-check");
    }

    #[test]
    fn test_file_storage_round_trip() {
        let temp_dir = std::env::temp_dir().join("sah_test_file_storage_round_trip");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let mut storage = FileStorage::new(&temp_dir);

        let rule = Rule::builder(
            "test-rule".to_string(),
            "Check for {{pattern}}".to_string(),
            Severity::Error,
        )
        .description("Test description".to_string())
        .category("testing".to_string())
        .tag("test".to_string())
        .tag("example".to_string())
        .auto_fix(true)
        .build();

        // Store the rule
        storage.store("test-rule", &rule).unwrap();

        // Retrieve the rule
        let retrieved = storage.get("test-rule").unwrap().unwrap();

        // Verify all fields match
        assert_eq!(retrieved.name, rule.name);
        assert_eq!(retrieved.template, rule.template);
        assert_eq!(retrieved.severity, rule.severity);
        assert_eq!(retrieved.description, rule.description);
        assert_eq!(retrieved.category, rule.category);
        assert_eq!(retrieved.tags, rule.tags);
        assert_eq!(retrieved.auto_fix, rule.auto_fix);

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_file_storage_complex_rule() {
        let temp_dir = std::env::temp_dir().join("sah_test_file_storage_complex");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let mut storage = FileStorage::new(&temp_dir);

        // Create a rule with all optional fields populated
        let rule = Rule::builder(
            "complex-rule".to_string(),
            "# Complex Template\n\nCheck for:\n- {{item1}}\n- {{item2}}".to_string(),
            Severity::Warning,
        )
        .description("A complex rule with many fields".to_string())
        .category("security".to_string())
        .tag("critical".to_string())
        .tag("production".to_string())
        .tag("automated".to_string())
        .auto_fix(false)
        .metadata_value("author".to_string(), serde_json::json!("test-user"))
        .metadata_value("version".to_string(), serde_json::json!("1.0.0"))
        .build();

        storage.store("complex-rule", &rule).unwrap();
        let retrieved = storage.get("complex-rule").unwrap().unwrap();

        assert_eq!(retrieved.name, rule.name);
        assert_eq!(retrieved.template, rule.template);
        assert_eq!(retrieved.severity, rule.severity);
        assert_eq!(retrieved.description, rule.description);
        assert_eq!(retrieved.category, rule.category);
        assert_eq!(retrieved.tags.len(), 3);
        assert!(retrieved.tags.contains(&"critical".to_string()));
        assert!(retrieved.tags.contains(&"production".to_string()));
        assert!(retrieved.tags.contains(&"automated".to_string()));
        assert!(!retrieved.auto_fix);
        assert_eq!(
            retrieved.metadata.get("author"),
            Some(&serde_json::json!("test-user"))
        );
        assert_eq!(
            retrieved.metadata.get("version"),
            Some(&serde_json::json!("1.0.0"))
        );

        std::fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_file_storage_list_keys() {
        let temp_dir = std::env::temp_dir().join("sah_test_file_storage_list");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let mut storage = FileStorage::new(&temp_dir);

        let rule1 = Rule::new(
            "rule1".to_string(),
            "Template 1".to_string(),
            Severity::Error,
        );
        let rule2 = Rule::new(
            "rule2".to_string(),
            "Template 2".to_string(),
            Severity::Warning,
        );
        let rule3 = Rule::new(
            "rule3".to_string(),
            "Template 3".to_string(),
            Severity::Info,
        );

        storage.store("rule1", &rule1).unwrap();
        storage.store("rule2", &rule2).unwrap();
        storage.store("rule3", &rule3).unwrap();

        let keys = storage.list_keys().unwrap();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"rule1".to_string()));
        assert!(keys.contains(&"rule2".to_string()));
        assert!(keys.contains(&"rule3".to_string()));

        std::fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_file_storage_remove() {
        let temp_dir = std::env::temp_dir().join("sah_test_file_storage_remove");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let mut storage = FileStorage::new(&temp_dir);

        let rule = Rule::new("test".to_string(), "Template".to_string(), Severity::Error);
        storage.store("test", &rule).unwrap();
        assert!(storage.exists("test").unwrap());

        let removed = storage.remove("test").unwrap();
        assert!(removed);
        assert!(!storage.exists("test").unwrap());

        let removed_again = storage.remove("test").unwrap();
        assert!(!removed_again);

        std::fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_file_storage_clear() {
        let temp_dir = std::env::temp_dir().join("sah_test_file_storage_clear");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let mut storage = FileStorage::new(&temp_dir);

        let rule1 = Rule::new(
            "rule1".to_string(),
            "Template 1".to_string(),
            Severity::Error,
        );
        let rule2 = Rule::new(
            "rule2".to_string(),
            "Template 2".to_string(),
            Severity::Warning,
        );

        storage.store("rule1", &rule1).unwrap();
        storage.store("rule2", &rule2).unwrap();
        assert_eq!(storage.count().unwrap(), 2);

        storage.clear().unwrap();
        assert_eq!(storage.count().unwrap(), 0);

        std::fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_file_storage_missing_frontmatter() {
        let temp_dir = std::env::temp_dir().join("sah_test_file_storage_missing_frontmatter");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let storage = FileStorage::new(&temp_dir);

        // Create a file without frontmatter
        let file_path = temp_dir.join("invalid.md");
        std::fs::write(&file_path, "Just content without frontmatter").unwrap();

        // Attempting to get should fail
        let result = storage.get("invalid");
        assert!(result.is_err());

        std::fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_file_storage_malformed_frontmatter() {
        let temp_dir = std::env::temp_dir().join("sah_test_file_storage_malformed");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let storage = FileStorage::new(&temp_dir);

        // Create a file with malformed YAML
        let file_path = temp_dir.join("malformed.md");
        std::fs::write(
            &file_path,
            "---\nname: test\ninvalid: yaml: content:\n---\nTemplate",
        )
        .unwrap();

        let result = storage.get("malformed");
        assert!(result.is_err());

        std::fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_file_storage_nonexistent_file() {
        let temp_dir = std::env::temp_dir().join("sah_test_file_storage_nonexistent");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let storage = FileStorage::new(&temp_dir);

        let result = storage.get("nonexistent");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        std::fs::remove_dir_all(&temp_dir).unwrap();
    }
}
