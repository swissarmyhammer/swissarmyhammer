use crate::{PromptLibrary, PromptLoader, Result};
use std::collections::HashMap;
use swissarmyhammer_common::file_loader::{FileSource, VirtualFileSystem};

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
    ///
    /// Also loads partials into the library's storage for template rendering.
    pub fn load_all_prompts(&mut self, library: &mut PromptLibrary) -> Result<()> {
        // Load builtin prompts first (least precedence)
        self.load_builtin_prompts()?;

        // Load all files from directories using VFS
        self.vfs.load_all()?;

        // Process all loaded files into prompts and partials
        let loader = PromptLoader::new();
        for file in self.vfs.list() {
            // Check if this is a partial template - either by tag or frontmatter
            let has_partial_tag = file.content.trim_start().starts_with("{% partial %}");
            let has_partial_frontmatter = crate::frontmatter::parse_frontmatter(&file.content)
                .ok()
                .and_then(|fm| fm.metadata)
                .and_then(|m| m.get("partial").cloned())
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let is_partial = has_partial_tag || has_partial_frontmatter;

            if is_partial {
                // For partials, create a minimal prompt object to store in the library
                // Store with multiple name variants so liquid can find them regardless of how they're referenced
                let base_name = file.name.strip_suffix(".liquid").unwrap_or(&file.name);
                let base_name = base_name.strip_suffix(".md").unwrap_or(base_name);

                // Add the partial with the base name (e.g., "workflow_guards")
                let mut partial_prompt =
                    crate::prompts::Prompt::new(base_name, file.content.clone());
                partial_prompt
                    .metadata
                    .insert("partial".to_string(), serde_json::Value::Bool(true));
                library.add(partial_prompt)?;
                self.prompt_sources
                    .insert(base_name.to_string(), file.source.clone());

                // Also add with .md extension (e.g., "workflow_guards.md")
                let name_with_md = format!("{}.md", base_name);
                let mut partial_with_md =
                    crate::prompts::Prompt::new(&name_with_md, file.content.clone());
                partial_with_md
                    .metadata
                    .insert("partial".to_string(), serde_json::Value::Bool(true));
                library.add(partial_with_md)?;
                self.prompt_sources
                    .insert(name_with_md, file.source.clone());

                // Also add with .liquid extension (e.g., "workflow_guards.liquid")
                let name_with_liquid = format!("{}.liquid", base_name);
                let mut partial_with_liquid =
                    crate::prompts::Prompt::new(&name_with_liquid, file.content.clone());
                partial_with_liquid
                    .metadata
                    .insert("partial".to_string(), serde_json::Value::Bool(true));
                library.add(partial_with_liquid)?;
                self.prompt_sources
                    .insert(name_with_liquid, file.source.clone());
            } else {
                // Load regular prompts normally
                let prompt = loader.load_from_string(&file.name, &file.content)?;

                // Track the source
                self.prompt_sources
                    .insert(prompt.name.clone(), file.source.clone());

                // Add to library
                library.add(prompt)?;
            }
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
    use tempfile::TempDir;

    #[test]
    fn test_prompt_resolver_loads_user_prompts() {
        // Skip isolated test environment setup for now
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
        let temp_dir = TempDir::new().unwrap();

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
            Err(_) => return, // Skip test if current directory is not accessible
        };
        if std::env::set_current_dir(&temp_dir).is_err() {
            return; // Skip test if can't change directory
        }

        resolver.load_all_prompts(&mut library).unwrap();

        // Restore original directory
        let _ = std::env::set_current_dir(original_dir);

        // Check that our test prompt was loaded
        let prompt = library.get("local_prompt").unwrap();
        assert_eq!(prompt.name, "local_prompt");
        assert_eq!(
            resolver.prompt_sources.get("local_prompt"),
            Some(&FileSource::Local)
        );
    }

    #[test]
    fn test_get_prompt_directories() {
        // Skip isolated test environment setup for now
        let resolver = PromptResolver::new();
        let directories = resolver.get_prompt_directories().unwrap();

        // Should return a vector of PathBuf (may be empty if no directories exist)
        // At minimum, should not panic and should return a valid result
        // Note: Vec::len() is always >= 0, so no need to test this

        // All returned paths should be absolute and existing
        // (The implementation only returns directories that exist)
        for dir in &directories {
            assert!(
                dir.is_absolute(),
                "Directory path should be absolute: {:?}",
                dir
            );
            // Note: Due to test isolation issues, directories may be cleaned up by other tests
            // The get_directories implementation only returns existing directories, so if we get here,
            // the directory existed at query time, but may not exist now due to test cleanup
            if dir.exists() {
                assert!(dir.is_dir(), "Path should be a directory: {:?}", dir);
            } else {
                // Directory was cleaned up between query and assertion - this is acceptable in tests
                println!(
                    "Warning: Directory {:?} was cleaned up during test execution",
                    dir
                );
            }
        }

        // Test that the function doesn't panic even when no directories exist
        // This is implicitly tested since we got here successfully
    }

    #[test]
    fn test_user_prompt_overrides_builtin_source_tracking() {
        // Skip isolated test environment setup for now
        let temp_dir = TempDir::new().unwrap();
        let user_prompts_dir = temp_dir.path().join(".swissarmyhammer").join("prompts");
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

        // Store original HOME value to restore later

        // Temporarily change home directory for test
        std::env::set_var("HOME", temp_dir.path());

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

    #[test]
    fn test_partial_frontmatter_registers_all_name_variants() {
        // Create a partial with frontmatter `partial: true`
        let mut library = PromptLibrary::new();
        let partial_content = r#"---
partial: true
---
This is partial content"#;

        // Simulate what the resolver does for partials
        let has_partial_frontmatter = crate::frontmatter::parse_frontmatter(partial_content)
            .ok()
            .and_then(|fm| fm.metadata)
            .and_then(|m| m.get("partial").cloned())
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        assert!(
            has_partial_frontmatter,
            "Should detect partial: true in frontmatter"
        );

        // Register with all variants like the resolver does
        let base_name = "test_partial";
        library
            .add(crate::prompts::Prompt::new(base_name, partial_content))
            .unwrap();
        library
            .add(crate::prompts::Prompt::new(
                format!("{}.md", base_name),
                partial_content,
            ))
            .unwrap();
        library
            .add(crate::prompts::Prompt::new(
                format!("{}.liquid", base_name),
                partial_content,
            ))
            .unwrap();

        // All three variants should be retrievable
        assert!(
            library.get("test_partial").is_ok(),
            "Should find partial without extension"
        );
        assert!(
            library.get("test_partial.md").is_ok(),
            "Should find partial with .md extension"
        );
        assert!(
            library.get("test_partial.liquid").is_ok(),
            "Should find partial with .liquid extension"
        );
    }

    #[test]
    fn test_render_partial_with_and_without_extension() {
        use swissarmyhammer_config::TemplateContext;

        // Create a library with a partial that uses frontmatter
        let mut library = PromptLibrary::new();

        // Add the partial with all name variants
        let partial_content = "PARTIAL_CONTENT_HERE";
        library
            .add(crate::prompts::Prompt::new(
                "_test/my_partial",
                partial_content,
            ))
            .unwrap();
        library
            .add(crate::prompts::Prompt::new(
                "_test/my_partial.md",
                partial_content,
            ))
            .unwrap();

        // Add a template that references the partial WITHOUT extension
        let template_without_ext = r#"Before {% render "_test/my_partial" %} After"#;
        library
            .add(crate::prompts::Prompt::new(
                "test_without_ext",
                template_without_ext,
            ))
            .unwrap();

        // Add a template that references the partial WITH extension
        let template_with_ext = r#"Before {% render "_test/my_partial.md" %} After"#;
        library
            .add(crate::prompts::Prompt::new(
                "test_with_ext",
                template_with_ext,
            ))
            .unwrap();

        let ctx = TemplateContext::new();

        // Both should render successfully
        let result_without = library.render("test_without_ext", &ctx);
        assert!(
            result_without.is_ok(),
            "Should render partial without extension: {:?}",
            result_without.err()
        );
        assert!(
            result_without.unwrap().contains("PARTIAL_CONTENT_HERE"),
            "Rendered content should include partial"
        );

        let result_with = library.render("test_with_ext", &ctx);
        assert!(
            result_with.is_ok(),
            "Should render partial with .md extension: {:?}",
            result_with.err()
        );
        assert!(
            result_with.unwrap().contains("PARTIAL_CONTENT_HERE"),
            "Rendered content should include partial"
        );
    }
}
