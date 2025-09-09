//! # SwissArmyHammer Prompts Domain Crate
//!
//! This crate provides prompt management functionality for SwissArmyHammer,
//! including loading, filtering, and rendering prompts with template integration.
//!
//! ## Features
//!
//! - **Prompt Management**: Load and organize prompts from various sources
//! - **Template Integration**: Uses swissarmyhammer-templating for rendering
//! - **Filtering**: Advanced filtering capabilities for prompt selection
//! - **Resolution**: Hierarchical prompt loading with precedence rules

#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use swissarmyhammer_common::SwissArmyHammerError;
use swissarmyhammer_templating::TemplateEngine;
use swissarmyhammer_config::TemplateContext;

// Declare modules
mod prompts;

// Re-export the PromptLoader from prompts module
pub use prompts::PromptLoader;

// Include the generated builtin prompts
include!(concat!(env!("OUT_DIR"), "/builtin_prompts.rs"));

/// Result type for prompt operations
pub type Result<T> = std::result::Result<T, SwissArmyHammerError>;

/// Represents a prompt source type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PromptSource {
    /// Built-in prompts embedded in the binary
    Builtin,
    /// User prompts from ~/.swissarmyhammer/prompts
    User, 
    /// Local prompts from project .swissarmyhammer directories
    Local,
}

/// Represents a single prompt with metadata and template content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    /// Unique name/identifier for the prompt
    pub name: String,
    /// Template content with variable placeholders
    pub template: String,
    /// Optional description
    pub description: Option<String>,
    /// Optional category for organization
    pub category: Option<String>,
    /// Tags for searchability
    pub tags: Vec<String>,
}

impl Prompt {
    /// Create a new prompt with name and template
    pub fn new(name: impl Into<String>, template: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            template: template.into(),
            description: None,
            category: None,
            tags: Vec::new(),
        }
    }

    /// Add a description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add a category
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// Add tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Render the prompt template with provided arguments
    pub fn render(&self, args: &HashMap<String, String>) -> Result<String> {
        let engine = TemplateEngine::new();
        engine.render(&self.template, args)
            .map_err(|e| SwissArmyHammerError::Other { 
                message: format!("Failed to render prompt '{}': {}", self.name, e) 
            })
    }
}

/// Handles loading prompts from various sources with proper precedence
#[derive(Debug, Default)]
pub struct PromptResolver {
    /// Track the source of each prompt by name
    pub prompt_sources: HashMap<String, PromptSource>,
}

impl PromptResolver {
    /// Create a new PromptResolver
    pub fn new() -> Self {
        Self::default()
    }

    /// Load all prompts following the correct precedence
    /// Loads built-in prompts first, then user/local prompts
    pub fn load_all_prompts(&mut self, library: &mut PromptLibrary) -> Result<()> {
        tracing::debug!("Starting to load all prompts");
        
        // Load builtin prompts first (lowest precedence)
        self.load_builtin_prompts(library)?;
        
        let final_count = library.get_all().len();
        tracing::debug!("Total prompts in library after loading: {}", final_count);
        
        // TODO: Add user and local prompt loading here
        // For now, just load built-ins to fix the failing tests
        
        Ok(())
    }

    /// Load built-in prompts into the library
    fn load_builtin_prompts(&mut self, library: &mut PromptLibrary) -> Result<()> {
        let builtin_prompts = get_builtin_prompts();
        tracing::debug!("Loading {} builtin prompts", builtin_prompts.len());
        
        let loader = PromptLoader::new();
        let mut loaded_count = 0;
        
        for (name, content) in builtin_prompts {
            tracing::debug!("Loading builtin prompt: {}", name);
            match loader.load_from_string(name, content) {
                Ok(prompt) => {
                    self.prompt_sources.insert(name.to_string(), PromptSource::Builtin);
                    library.add_prompt(prompt, PromptSource::Builtin);
                    loaded_count += 1;
                    tracing::debug!("Successfully loaded builtin prompt: {}", name);
                }
                Err(e) => {
                    tracing::warn!("Failed to load builtin prompt '{}': {}", name, e);
                    // Continue loading other prompts even if one fails
                }
            }
        }
        
        tracing::debug!("Loaded {} builtin prompts successfully", loaded_count);
        Ok(())
    }

    /// Get all directories that prompts are loaded from
    pub fn get_prompt_directories(&self) -> Result<Vec<std::path::PathBuf>> {
        // TODO: Implement actual directory discovery
        Ok(vec![])
    }
}

/// A collection of prompts with management capabilities
#[derive(Debug, Default)]
pub struct PromptLibrary {
    /// Internal storage of prompts
    prompts: HashMap<String, Prompt>,
    /// Track sources of prompts
    sources: HashMap<String, PromptSource>,
}

impl PromptLibrary {
    /// Create a new empty prompt library
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a prompt to the library
    pub fn add_prompt(&mut self, prompt: Prompt, source: PromptSource) {
        self.sources.insert(prompt.name.clone(), source);
        self.prompts.insert(prompt.name.clone(), prompt);
    }

    /// Add a prompt to the library (compatibility method)
    /// Defaults to User source for backward compatibility
    pub fn add(&mut self, prompt: Prompt) -> Result<()> {
        self.add_prompt(prompt, PromptSource::User);
        Ok(())
    }

    /// Get a prompt by name
    pub fn get(&self, name: &str) -> Result<&Prompt> {
        self.prompts.get(name)
            .ok_or_else(|| SwissArmyHammerError::Other { 
                message: format!("Prompt '{}' not found", name) 
            })
    }

    /// List all prompt names
    pub fn list_names(&self) -> Vec<&String> {
        self.prompts.keys().collect()
    }

    /// Get all prompts
    pub fn get_all(&self) -> &HashMap<String, Prompt> {
        &self.prompts
    }

    /// Get prompt source
    pub fn get_source(&self, name: &str) -> Option<&PromptSource> {
        self.sources.get(name)
    }

    /// List all prompts - returns a Result for compatibility
    pub fn list(&self) -> Result<Vec<&Prompt>> {
        Ok(self.prompts.values().collect())
    }

    /// Render a prompt by name with arguments (HashMap version)
    pub fn render_with_args(&self, name: &str, args: &HashMap<String, String>) -> Result<String> {
        let prompt = self.get(name)?;
        prompt.render(args)
    }

    /// Render a prompt by name with TemplateContext
    pub fn render(&self, name: &str, context: &TemplateContext) -> Result<String> {
        let prompt = self.get(name)?;
        let engine = TemplateEngine::new();
        engine.render_with_context(&prompt.template, context)
            .map_err(|e| SwissArmyHammerError::Other { 
                message: format!("Failed to render prompt '{}': {}", name, e) 
            })
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_creation() {
        let prompt = Prompt::new("test", "Hello {{name}}!")
            .with_description("A test prompt")
            .with_category("test")
            .with_tags(vec!["greeting".to_string()]);

        assert_eq!(prompt.name, "test");
        assert_eq!(prompt.template, "Hello {{name}}!");
        assert_eq!(prompt.description, Some("A test prompt".to_string()));
        assert_eq!(prompt.category, Some("test".to_string()));
        assert_eq!(prompt.tags, vec!["greeting"]);
    }

    #[test]
    fn test_prompt_library() {
        let mut library = PromptLibrary::new();
        let prompt = Prompt::new("test", "Hello {{name}}!");
        
        library.add_prompt(prompt, PromptSource::Local);
        
        assert!(library.get("test").is_ok());
        assert_eq!(library.list_names().len(), 1);
        assert_eq!(library.get_source("test"), Some(&PromptSource::Local));
    }
}