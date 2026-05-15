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
mod frontmatter;
mod prompt_filter;
mod prompt_partial_adapter;
mod prompt_resolver;
mod prompts;
mod storage;

// Re-export main types from prompts module
pub use prompts::{Prompt, PromptLibrary, PromptLoader};

// Re-export prompt resolver
pub use prompt_resolver::PromptResolver;

// Re-export storage types
pub use storage::{FileStorage, MemoryStorage, StorageBackend};

// Re-export frontmatter types
pub use frontmatter::{parse_frontmatter, FrontmatterResult};

// Re-export filter types
pub use prompt_filter::PromptFilter;

// Re-export partial adapter types
pub use prompt_partial_adapter::{new_prompt_partial_adapter, PromptPartialAdapter};

// Include the generated builtin prompts
include!(concat!(env!("OUT_DIR"), "/builtin_prompts.rs"));

/// Result type for prompt operations
pub type Result<T> = std::result::Result<T, SwissArmyHammerError>;

/// Represents a prompt source type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PromptSource {
    /// Built-in prompts embedded in the binary
    Builtin,
    /// User prompts from ~/.prompts
    User,
    /// Local prompts from project .prompts directory
    Local,
}

impl From<swissarmyhammer_common::FileSource> for PromptSource {
    fn from(source: swissarmyhammer_common::FileSource) -> Self {
        match source {
            swissarmyhammer_common::FileSource::Builtin => PromptSource::Builtin,
            swissarmyhammer_common::FileSource::User => PromptSource::User,
            swissarmyhammer_common::FileSource::Local => PromptSource::Local,
            swissarmyhammer_common::FileSource::Dynamic => PromptSource::User, // Map Dynamic to User
        }
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

        library.add(prompt).unwrap();

        assert!(library.get("test").is_ok());
        assert_eq!(library.list_names().unwrap().len(), 1);
        // Note: get_source is not available in this simplified API
        // assert_eq!(library.get_source("test"), Some(&PromptSource::Local));
    }

    #[test]
    fn test_prompt_source_from_builtin() {
        let source: PromptSource = swissarmyhammer_common::FileSource::Builtin.into();
        assert_eq!(source, PromptSource::Builtin);
    }

    #[test]
    fn test_prompt_source_from_user() {
        let source: PromptSource = swissarmyhammer_common::FileSource::User.into();
        assert_eq!(source, PromptSource::User);
    }

    #[test]
    fn test_prompt_source_from_local() {
        let source: PromptSource = swissarmyhammer_common::FileSource::Local.into();
        assert_eq!(source, PromptSource::Local);
    }

    #[test]
    fn test_prompt_source_from_dynamic() {
        let source: PromptSource = swissarmyhammer_common::FileSource::Dynamic.into();
        assert_eq!(source, PromptSource::User); // Dynamic maps to User
    }

    #[test]
    fn test_prompt_source_debug_and_clone() {
        let source = PromptSource::Builtin;
        let cloned = source.clone();
        assert_eq!(source, cloned);
        let debug = format!("{:?}", source);
        assert_eq!(debug, "Builtin");
    }

    #[test]
    fn test_prompt_source_serialize_deserialize() {
        let source = PromptSource::User;
        let json = serde_json::to_string(&source).unwrap();
        let deserialized: PromptSource = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, source);
    }

    #[test]
    fn test_prompt_source_all_variants() {
        // Ensure all three variants exist and are distinct
        assert_ne!(PromptSource::Builtin, PromptSource::User);
        assert_ne!(PromptSource::User, PromptSource::Local);
        assert_ne!(PromptSource::Builtin, PromptSource::Local);
    }
}
