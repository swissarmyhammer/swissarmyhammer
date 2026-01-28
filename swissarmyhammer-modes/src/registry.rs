//! Mode registry for discovering and loading modes
//!
//! This module provides the ModeRegistry for loading modes from the filesystem
//! using the standard SwissArmyHammer override stack.

use crate::{Mode, Result};
use std::collections::HashMap;
use swissarmyhammer_common::file_loader::VirtualFileSystem;
use swissarmyhammer_common::Pretty;

/// Registry for managing and loading modes
///
/// The [`ModeRegistry`] uses the standard SwissArmyHammer VirtualFileSystem
/// to load mode definitions with proper precedence:
/// 1. Builtin modes (embedded in binary)
/// 2. User modes (~/.swissarmyhammer/modes/)
/// 3. Local modes (.swissarmyhammer/modes/ in Git root)
///
/// # Examples
///
/// ```
/// use swissarmyhammer_modes::ModeRegistry;
///
/// let mut registry = ModeRegistry::new();
/// let modes = registry.load_all().unwrap();
///
/// for mode in &modes {
///     println!("{}: {}", mode.id(), mode.description());
/// }
/// ```
pub struct ModeRegistry {
    /// Virtual file system for loading mode files
    vfs: VirtualFileSystem,
    /// Loaded modes indexed by ID
    modes: HashMap<String, Mode>,
}

impl ModeRegistry {
    /// Create a new empty mode registry
    pub fn new() -> Self {
        let mut vfs = VirtualFileSystem::new("modes");

        // Load builtin modes into VFS
        for (id, content) in crate::builtin_modes() {
            vfs.add_builtin(id, content);
        }

        Self {
            vfs,
            modes: HashMap::new(),
        }
    }

    /// Load all modes using the standard SwissArmyHammer override stack
    ///
    /// Uses VirtualFileSystem to load from (in precedence order):
    /// 1. Built-in modes (embedded in binary) - lowest priority
    /// 2. User `~/.swissarmyhammer/modes/` - overrides builtin
    /// 3. Project `.swissarmyhammer/modes/` - highest priority, overrides all
    ///
    /// This follows the same override stack as prompts, rules, and other resources.
    ///
    /// Returns the loaded modes as a vector.
    pub fn load_all(&mut self) -> Result<Vec<Mode>> {
        // Load files from all sources using VirtualFileSystem
        self.vfs.load_all()?;

        // Parse each file as a mode
        for file in self.vfs.list() {
            match Mode::from_markdown(&file.content, &file.name) {
                Ok(mut mode) => {
                    mode = mode.with_source_path(file.path.clone());
                    tracing::debug!("Loaded mode: {} from {}", mode.id(), Pretty(&file.source));
                    self.modes.insert(mode.id().to_string(), mode);
                }
                Err(e) => {
                    tracing::warn!("Failed to parse mode from {}: {}", Pretty(&file.path), e);
                }
            }
        }

        Ok(self.modes.values().cloned().collect())
    }

    /// Get a mode by ID
    pub fn get(&self, id: &str) -> Option<&Mode> {
        self.modes.get(id)
    }

    /// List all mode IDs
    pub fn list_ids(&self) -> Vec<String> {
        self.modes.keys().cloned().collect()
    }

    /// Get all modes
    pub fn all_modes(&self) -> Vec<&Mode> {
        self.modes.values().collect()
    }

    /// Check if a mode exists
    pub fn contains(&self, id: &str) -> bool {
        self.modes.contains_key(id)
    }

    /// Get the number of loaded modes
    pub fn count(&self) -> usize {
        self.modes.len()
    }

    /// Add a mode to the registry
    pub fn add(&mut self, mode: Mode) {
        self.modes.insert(mode.id().to_string(), mode);
    }
}

impl Default for ModeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_registry_new() {
        let registry = ModeRegistry::new();
        // Registry is empty until load_all() is called
        assert_eq!(registry.count(), 0);
        assert!(registry.list_ids().is_empty());
    }

    #[test]
    fn test_mode_registry_add() {
        let mut registry = ModeRegistry::new();
        let mode = Mode::new("test", "Test Mode", "A test mode", "You are a test agent.");

        registry.add(mode);
        assert!(registry.contains("test"));
        assert!(registry.get("test").is_some());
    }

    #[test]
    fn test_mode_registry_load_all_includes_builtins() {
        let mut registry = ModeRegistry::new();
        let modes = registry.load_all().unwrap();

        // Should have at least the builtin modes
        assert!(modes.len() >= 3, "Should have at least 3 builtin modes");

        // Check for specific builtin modes
        let mode_ids: Vec<&str> = modes.iter().map(|m| m.id()).collect();
        assert!(
            mode_ids.contains(&"general-purpose"),
            "Should have general-purpose mode"
        );
        assert!(mode_ids.contains(&"Explore"), "Should have Explore mode");
        assert!(mode_ids.contains(&"Plan"), "Should have Plan mode");
    }

    #[test]
    fn test_mode_registry_override_precedence() {
        // This test verifies the override stack: builtin → user → project
        // We can't easily test the full stack without mocking directories,
        // but we can verify that later modes override earlier ones

        let mut registry = ModeRegistry::new();

        // Add a builtin-style mode
        let builtin_mode = Mode::new(
            "test-mode",
            "Test Mode (Builtin)",
            "Builtin version",
            "Builtin system prompt",
        );
        registry.add(builtin_mode);

        // Verify builtin is loaded
        assert_eq!(
            registry.get("test-mode").unwrap().name(),
            "Test Mode (Builtin)"
        );

        // Override with a user-style mode
        let user_mode = Mode::new(
            "test-mode",
            "Test Mode (User)",
            "User version",
            "User system prompt",
        );
        registry.add(user_mode);

        // Verify user version overrode builtin
        assert_eq!(
            registry.get("test-mode").unwrap().name(),
            "Test Mode (User)"
        );
        assert_eq!(
            registry.get("test-mode").unwrap().system_prompt(),
            "User system prompt"
        );
    }

    #[test]
    fn test_mode_registry_list_ids() {
        let mut registry = ModeRegistry::new();
        registry.add(Mode::new("mode1", "Mode 1", "First mode", "Prompt 1"));
        registry.add(Mode::new("mode2", "Mode 2", "Second mode", "Prompt 2"));

        let ids = registry.list_ids();
        assert!(ids.len() >= 2);
        assert!(ids.contains(&"mode1".to_string()));
        assert!(ids.contains(&"mode2".to_string()));
    }

    #[test]
    fn test_mode_registry_all_modes() {
        let mut registry = ModeRegistry::new();
        registry.add(Mode::new("mode1", "Mode 1", "First", "Prompt 1"));
        registry.add(Mode::new("mode2", "Mode 2", "Second", "Prompt 2"));

        let modes = registry.all_modes();
        assert!(modes.len() >= 2);
    }

    #[test]
    fn test_builtin_modes_are_embedded() {
        // Verify builtin_modes() function returns the expected modes
        let builtins = crate::builtin_modes();

        let ids: Vec<&str> = builtins.iter().map(|(id, _)| *id).collect();

        // Define expected modes explicitly
        // Note: rule-checker mode removed as part of swissarmyhammer-rules crate removal
        let expected_modes = [
            "general-purpose",
            "Explore",
            "Plan",
            "default",
            "planner",
            "implementer",
            "reviewer",
            "tester",
            "committer",
        ];

        assert_eq!(
            builtins.len(),
            expected_modes.len(),
            "Should have exactly {} builtin modes",
            expected_modes.len()
        );

        // Original embedded modes
        assert!(
            ids.contains(&"general-purpose"),
            "Should have general-purpose"
        );
        assert!(ids.contains(&"Explore"), "Should have Explore");
        assert!(ids.contains(&"Plan"), "Should have Plan");

        // New prompt-referencing modes
        assert!(ids.contains(&"default"), "Should have default");
        assert!(ids.contains(&"planner"), "Should have planner");
        assert!(ids.contains(&"implementer"), "Should have implementer");
        assert!(ids.contains(&"reviewer"), "Should have reviewer");
        assert!(ids.contains(&"tester"), "Should have tester");
        assert!(ids.contains(&"committer"), "Should have committer");
        // Note: rule-checker mode removed as part of swissarmyhammer-rules crate removal

        // Verify each has content
        for (id, content) in builtins {
            assert!(!content.is_empty(), "Mode {} should have content", id);
            assert!(
                content.contains("---"),
                "Mode {} should have frontmatter",
                id
            );
        }
    }
}
