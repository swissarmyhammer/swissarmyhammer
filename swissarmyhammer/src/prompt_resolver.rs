use crate::file_loader::{FileSource, VirtualFileSystem};
use crate::{PromptLibrary, PromptLoader, Result};
use std::collections::HashMap;

// Include the generated builtin prompts
include!(concat!(env!("OUT_DIR"), "/builtin_prompts.rs"));

/// Handles loading prompts from various sources with proper precedence
pub struct PromptResolver {
    /// Track the source of each prompt by name
    pub prompt_sources: HashMap<String, FileSource>,
    /// Virtual file system for managing prompts
    vfs: VirtualFileSystem,
}

impl PromptResolver {
    /// Create a new PromptResolver
    pub fn new() -> Self {
        Self {
            prompt_sources: HashMap::new(),
            vfs: VirtualFileSystem::new("prompts"),
        }
    }

    /// Get all directories that prompts are loaded from
    /// Returns paths in the same order as loading precedence
    pub fn get_prompt_directories(&self) -> Result<Vec<std::path::PathBuf>> {
        self.vfs.get_directories()
    }

    /// Load all prompts following the correct precedence:
    /// 1. Builtin prompts (least specific, embedded in binary)
    /// 2. User prompts from ~/.swissarmyhammer/prompts
    /// 3. Local prompts from .swissarmyhammer directories (most specific)
    pub fn load_all_prompts(&mut self, library: &mut PromptLibrary) -> Result<()> {
        // Load builtin prompts first (least precedence)
        self.load_builtin_prompts()?;

        // Load all files from directories using VFS
        self.vfs.load_all()?;

        // Process all loaded files into prompts
        let loader = PromptLoader::new();
        for file in self.vfs.list() {
            // Load the prompt from content
            let prompt = loader.load_from_string(&file.name, &file.content)?;

            // Track the source
            self.prompt_sources
                .insert(prompt.name.clone(), file.source.clone());

            // Add to library
            library.add(prompt)?;
        }

        Ok(())
    }

    /// Load builtin prompts from embedded binary data
    fn load_builtin_prompts(&mut self) -> Result<()> {
        let builtin_prompts = get_builtin_prompts();

        // Add builtin prompts to VFS
        for (name, content) in builtin_prompts {
            self.vfs.add_builtin(name, content);
        }

        Ok(())
    }
}

impl Default for PromptResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    

    #[test]
    fn test_prompt_resolver_loads_user_prompts() {
        let _guard = crate::test_utils::IsolatedTestEnvironment::new()
            .expect("Failed to create isolated test environment");
        let home_dir = std::env::var("HOME").unwrap();
        let user_prompts_dir = PathBuf::from(&home_dir)
            .join(".swissarmyhammer")
            .join("prompts");
        fs::create_dir_all(&user_prompts_dir).unwrap();

        // Create a test prompt file
        let prompt_file = user_prompts_dir.join("test_prompt.md");
        fs::write(&prompt_file, "This is a test prompt").unwrap();

        let mut resolver = PromptResolver::new();
        let mut library = PromptLibrary::new();

        resolver.load_all_prompts(&mut library).unwrap();

        // Check that our test prompt was loaded
        let prompt = library.get("test_prompt").unwrap();
        assert_eq!(prompt.name, "test_prompt");
        assert_eq!(
            resolver.prompt_sources.get("test_prompt"),
            Some(&FileSource::User)
        );
    }

    #[test]
    fn test_prompt_resolver_loads_local_prompts() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();

        // Create a .git directory to make it look like a Git repository
        let git_dir = temp_dir.path().join(".git");
        fs::create_dir_all(&git_dir).unwrap();

        let local_prompts_dir = temp_dir.path().join(".swissarmyhammer").join("prompts");
        fs::create_dir_all(&local_prompts_dir).unwrap();

        // Create a test prompt file with proper header
        let prompt_file = local_prompts_dir.join("local_prompt.md");
        fs::write(
            &prompt_file,
            "# Local Test Prompt\n\nThis is a local prompt for testing",
        )
        .unwrap();

        let mut resolver = PromptResolver::new();
        let mut library = PromptLibrary::new();

        // Change to the temp directory to simulate local prompts
        let original_dir = match std::env::current_dir() {
            Ok(dir) => dir,
            Err(_) => {
                // Current directory is invalid, skip this test
                return;
            }
        };

        if std::env::set_current_dir(&temp_dir).is_err() {
            // Failed to change directory, skip this test
            return;
        }

        resolver.load_all_prompts(&mut library).unwrap();

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();

        // Check that our test prompt was loaded
        let prompt = library.get("local_prompt").unwrap();
        assert_eq!(prompt.name, "local_prompt");
        assert_eq!(
            resolver.prompt_sources.get("local_prompt"),
            Some(&FileSource::Local)
        );
    }

    #[test]
    fn test_debug_error_prompt_is_correctly_tracked_as_builtin() {
        let mut resolver = PromptResolver::new();
        let mut library = PromptLibrary::new();

        // Load builtin prompts
        resolver.load_all_prompts(&mut library).unwrap();

        // The debug/error prompt should be loaded and tracked as builtin
        // First check that it exists in the library
        let prompts = library.list().unwrap();
        let debug_error_prompt = prompts.iter().find(|p| p.name == "debug/error");

        if let Some(_prompt) = debug_error_prompt {
            // Check that it's tracked as a builtin
            assert_eq!(
                resolver.prompt_sources.get("debug/error"),
                Some(&FileSource::Builtin),
                "debug/error prompt should be tracked as Builtin, but was tracked as: {:?}",
                resolver.prompt_sources.get("debug/error")
            );
        } else {
            // If debug/error doesn't exist, check if debug-error exists instead
            let debug_hyphen_error_prompt = prompts.iter().find(|p| p.name == "debug-error");
            if let Some(_prompt) = debug_hyphen_error_prompt {
                // This would indicate the bug where frontmatter name overrides build script name
                panic!("Found prompt named 'debug-error' instead of 'debug/error'. This indicates the frontmatter is overriding the build script name.");
            } else {
                // Check what builtin prompts actually exist
                let builtin_prompt_names: Vec<String> =
                    prompts.iter().map(|p| p.name.clone()).collect();
                panic!(
                    "debug/error prompt not found. Available builtin prompts: {builtin_prompt_names:?}"
                );
            }
        }
    }

    #[test]
    fn test_get_prompt_directories() {
        let resolver = PromptResolver::new();
        let directories = resolver.get_prompt_directories().unwrap();

        // Should return a vector of PathBuf (may be empty if no directories exist)
        // At minimum, should not panic and should return a valid result
        // Note: Vec::len() is always >= 0, so no need to test this

        // All returned paths should be absolute and existing
        for dir in directories {
            assert!(dir.is_absolute());
            assert!(dir.exists());
            assert!(dir.is_dir());
        }
    }

    #[test]
    fn test_user_prompt_overrides_builtin_source_tracking() {
        let guard = crate::test_utils::IsolatedTestEnvironment::new()
            .expect("Failed to create isolated test environment");
        let user_prompts_dir = guard.home_path().join(".swissarmyhammer").join("prompts");
        fs::create_dir_all(&user_prompts_dir).unwrap();

        // Create a user prompt with the same name as a builtin prompt
        let prompt_file = user_prompts_dir.join("debug").join("error.md");
        fs::create_dir_all(prompt_file.parent().unwrap()).unwrap();
        let user_prompt_content = r"---
title: User Debug Error
description: User-defined error debugging prompt
---

This is a user-defined debug/error prompt that should override the builtin one.
";
        fs::write(&prompt_file, user_prompt_content).unwrap();

        let mut resolver = PromptResolver::new();
        let mut library = PromptLibrary::new();

        // Load builtin prompts first
        resolver.load_all_prompts(&mut library).unwrap();

        // Check if debug/error exists as builtin (it might not always exist)
        let has_builtin_debug_error = resolver.prompt_sources.contains_key("debug/error");

        // Load user prompts (should override the builtin if it exists, or just add it if not)
        resolver.load_all_prompts(&mut library).unwrap();

        // Now it should be tracked as a user prompt
        assert_eq!(
            resolver.prompt_sources.get("debug/error"),
            Some(&FileSource::User),
            "debug/error should be tracked as User prompt after loading user prompts"
        );

        // Verify the prompt content was updated/loaded
        let prompt = library.get("debug/error").unwrap();
        assert!(
            prompt.template.contains("user-defined"),
            "Prompt should contain user-defined content"
        );

        // HOME is automatically restored when _guard goes out of scope

        // If we had a builtin debug/error, verify it was actually overridden
        if has_builtin_debug_error {
            assert_eq!(
                resolver.prompt_sources.get("debug/error"),
                Some(&FileSource::User),
                "Builtin debug/error should have been overridden by user prompt"
            );
        }
    }
}
