use crate::{Result, Rule, RuleLoader};
use std::collections::HashMap;
use swissarmyhammer_common::file_loader::{FileSource, VirtualFileSystem};

// Include the generated builtin rules
include!(concat!(env!("OUT_DIR"), "/builtin_rules.rs"));

/// Handles loading rules from various sources with proper precedence
pub struct RuleResolver {
    /// Track the source of each rule by name
    pub rule_sources: HashMap<String, FileSource>,
    /// Virtual file system for managing rules
    vfs: VirtualFileSystem,
}

impl RuleResolver {
    /// Create a new RuleResolver
    pub fn new() -> Self {
        Self {
            rule_sources: HashMap::new(),
            vfs: VirtualFileSystem::new("rules"),
        }
    }

    /// Get all directories that rules are loaded from
    /// Returns paths in the same order as loading precedence
    pub fn get_rule_directories(&self) -> Result<Vec<std::path::PathBuf>> {
        self.vfs.get_directories()
    }

    /// Load all rules following the correct precedence:
    /// 1. Builtin rules (least specific, embedded in binary)
    /// 2. User rules from ~/.swissarmyhammer/rules
    /// 3. Local rules from .swissarmyhammer directories (most specific)
    ///
    /// Higher precedence rules override lower ones by name.
    pub fn load_all_rules(&mut self, rules: &mut Vec<Rule>) -> Result<()> {
        // Load builtin rules first (least precedence)
        self.load_builtin_rules()?;

        // Load all files from directories using VFS
        self.vfs.load_all()?;

        // Process all loaded files into rules
        let loader = RuleLoader::new();
        for file in self.vfs.list() {
            // Load the rule
            let mut rule = loader.load_from_string(&file.name, &file.content)?;

            // Set the source path
            rule.source = Some(file.path.clone());

            // Track the source
            self.rule_sources
                .insert(rule.name.clone(), file.source.clone());

            // Add to rules list (later rules override earlier ones with same name)
            if let Some(pos) = rules.iter().position(|r| r.name == rule.name) {
                // Replace existing rule with same name (higher precedence)
                rules[pos] = rule;
            } else {
                // Add new rule
                rules.push(rule);
            }
        }

        // Sort rules by name for consistent ordering
        rules.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(())
    }

    /// Load builtin rules from embedded binary data
    fn load_builtin_rules(&mut self) -> Result<()> {
        let builtin_rules = get_builtin_rules();

        // Add builtin rules to VFS
        for (name, content) in builtin_rules {
            self.vfs.add_builtin(name, content);
        }

        Ok(())
    }
}

impl Default for RuleResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_rule_resolver_loads_builtin_rules() {
        let mut resolver = RuleResolver::new();
        let mut rules = Vec::new();

        resolver.load_all_rules(&mut rules).unwrap();

        // Should have loaded some builtin rules (even if empty for now)
        // The key is that it doesn't error
    }

    #[test]
    fn test_rule_resolver_precedence() {
        let temp_dir = TempDir::new().unwrap();

        // Create a .git directory to make it look like a Git repository
        let git_dir = temp_dir.path().join(".git");
        fs::create_dir_all(&git_dir).unwrap();

        let local_rules_dir = temp_dir.path().join(".swissarmyhammer").join("rules");
        fs::create_dir_all(&local_rules_dir).unwrap();

        // Create a test rule file that would override a builtin
        let rule_file = local_rules_dir.join("test-rule.md");
        fs::write(
            &rule_file,
            r#"---
title: Local Test Rule
description: This is a local rule for testing
severity: error
---

Check for local issues
"#,
        )
        .unwrap();

        let mut resolver = RuleResolver::new();
        let mut rules = Vec::new();

        // Change to the temp directory to simulate local rules
        let original_dir = match std::env::current_dir() {
            Ok(dir) => dir,
            Err(_) => return, // Skip test if current directory is not accessible
        };
        if std::env::set_current_dir(&temp_dir).is_err() {
            return; // Skip test if can't change directory
        }

        resolver.load_all_rules(&mut rules).unwrap();

        // Restore original directory
        let _ = std::env::set_current_dir(original_dir);

        // Check that our test rule was loaded
        let test_rule = rules.iter().find(|r| r.name == "test-rule");
        assert!(test_rule.is_some());

        let test_rule = test_rule.unwrap();
        assert_eq!(test_rule.name, "test-rule");
        assert_eq!(
            resolver.rule_sources.get("test-rule"),
            Some(&FileSource::Local)
        );
    }

    #[test]
    fn test_get_rule_directories() {
        let resolver = RuleResolver::new();
        let directories = resolver.get_rule_directories().unwrap();

        // Should return a vector of PathBuf (may be empty if no directories exist)

        // All returned paths should be absolute
        for dir in &directories {
            assert!(
                dir.is_absolute(),
                "Directory path should be absolute: {:?}",
                dir
            );
        }
    }

    #[test]
    fn test_rule_resolver_source_tracking() {
        let mut resolver = RuleResolver::new();
        let mut rules = Vec::new();

        resolver.load_all_rules(&mut rules).unwrap();

        // All loaded rules should have their sources tracked
        for rule in &rules {
            assert!(
                resolver.rule_sources.contains_key(&rule.name),
                "Rule '{}' should have source tracked",
                rule.name
            );
        }
    }
}
