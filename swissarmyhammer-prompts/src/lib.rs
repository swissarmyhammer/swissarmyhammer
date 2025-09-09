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
use swissarmyhammer_common::SwissArmyHammerError;

// Declare modules
mod prompts;
mod parameter_types;
mod storage;
mod validation;
mod frontmatter;
mod prompt_filter;
mod prompt_partial_adapter;
mod prompt_resolver;
mod file_loader;

// Re-export main types from prompts module
pub use prompts::{Prompt, PromptLibrary, PromptLoader};

// Re-export prompt resolver
pub use prompt_resolver::PromptResolver;

// Re-export file loader types
pub use file_loader::{FileSource, FileEntry, VirtualFileSystem};

// Re-export validation types
pub use validation::{ValidationIssue, ValidationLevel, Validatable, ValidationResult};

// Re-export parameter types
pub use parameter_types::{Parameter, ParameterProvider, ParameterType};

// Re-export storage types
pub use storage::{StorageBackend, MemoryStorage, FileStorage};

// Re-export frontmatter types
pub use frontmatter::{parse_frontmatter, FrontmatterResult};

// Re-export filter types
pub use prompt_filter::PromptFilter;

// Re-export partial adapter types
pub use prompt_partial_adapter::PromptPartialAdapter;

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
        
        library.add(prompt).unwrap();
        
        assert!(library.get("test").is_ok());
        assert_eq!(library.list_names().unwrap().len(), 1);
        // Note: get_source is not available in this simplified API
        // assert_eq!(library.get_source("test"), Some(&PromptSource::Local));
    }
}