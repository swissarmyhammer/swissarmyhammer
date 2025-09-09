//! # `SwissArmyHammer`
//!
//! A flexible prompt management library for AI assistants.
//!
//! ## Features
//!
//! - **Prompt Management**: Load, store, and organize prompts from various sources
//! - **Template Engine**: Powerful Liquid-based template processing
//! - **Semantic Search**: Vector-based semantic search for source code files
//! - **MCP Support**: Model Context Protocol server integration
//! - **Async/Sync APIs**: Choose between async and sync interfaces
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use swissarmyhammer::PromptLibrary;
//! use std::collections::HashMap;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a new prompt library
//! let mut library = PromptLibrary::new();
//!
//! // Add prompts from a directory
//! if std::path::Path::new("./.swissarmyhammer/prompts").exists() {
//!     library.add_directory("./.swissarmyhammer/prompts")?;
//! }
//!
//! // Get a prompt and render it
//! let prompt = library.get("code-review")?;
//! let mut args = HashMap::new();
//! args.insert("language".to_string(), "rust".to_string());
//! args.insert("file".to_string(), "main.rs".to_string());
//! let rendered = prompt.render(&args)?;
//!
//! println!("{}", rendered);
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

/// Prompt management and storage
pub mod prompts;

/// Prompt filtering functionality
pub mod prompt_filter;

/// Prompt loading and resolution
pub mod prompt_resolver;

/// Template engine and rendering (legacy - being migrated to swissarmyhammer-templating)
pub mod template;

/// Adapter to make PromptLibrary work with new templating domain crate
pub mod prompt_partial_adapter;

/// Storage abstractions and implementations
pub mod storage;

/// Semantic search functionality with vector embeddings
pub mod search;

/// Outline generation functionality for Tree-sitter based code analysis
pub mod outline;

/// Plugin system for extensibility
pub mod plugins;

/// Workflow system for state-based execution
pub mod workflow;

/// Shared frontmatter parsing functionality
pub mod frontmatter;

/// Security utilities for path validation and resource limits
pub mod security;

/// File watching functionality for prompt directories
pub mod file_watcher;

/// Virtual file system for unified file loading
pub mod file_loader;

/// Unified file system utilities for better error handling and testing
pub mod fs_utils;

/// Validation framework for checking content integrity
pub mod validation;

// Re-export core types

/// File source for loading prompts from various sources
pub use file_loader::FileSource;

/// File system utilities and abstractions
pub use fs_utils::{FilePermissions, FileSystem, FileSystemUtils};

/// Plan command utilities
pub mod plan_utils;

/// Plugin system types for extending functionality
pub use plugins::{CustomLiquidFilter, PluginRegistry, SwissArmyHammerPlugin};

/// Prompt filtering functionality
pub use prompt_filter::PromptFilter;

/// Advanced prompt loading and resolution
pub use prompt_resolver::PromptResolver;

/// Backward compatibility alias for FileSource
pub use file_loader::FileSource as PromptSource;

/// Core prompt management types and functionality
pub use prompts::{Prompt, PromptLibrary, PromptLoader};

/// Storage backends and abstractions
pub use storage::{PromptStorage, StorageBackend};

/// Template engine and rendering functionality (re-exported from swissarmyhammer-templating)
pub use template::{Template, TemplateEngine};

/// Workflow system for state-based execution
pub use workflow::{
    State, StateId, Transition, Workflow, WorkflowName, WorkflowRun, WorkflowRunId,
    WorkflowRunStatus,
};

/// Validation types and traits
pub use validation::{Validatable, ValidationIssue, ValidationLevel, ValidationResult};

// sah.toml configuration types removed (migrated to swissarmyhammer-config)
// All TOML configuration functionality now provided by swissarmyhammer-config crate using figment

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Error types used throughout the library
pub mod error;

// sah.toml configuration support removed (migrated to swissarmyhammer-config)

// pub mod toml_core; // REMOVED - All TOML functionality moved to swissarmyhammer-config

pub use error::{ErrorChainExt, ErrorContext, Result, SwissArmyHammerError};

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::{
        CustomLiquidFilter, FileSystem, FileSystemUtils, PluginRegistry, Prompt, PromptLibrary,
        PromptLoader, PromptStorage, Result, StorageBackend, SwissArmyHammerError,
        SwissArmyHammerPlugin, Template, TemplateEngine,
    };

    pub use crate::workflow::{
        State, StateId, Transition, Workflow, WorkflowName, WorkflowRun, WorkflowRunId,
        WorkflowRunStatus,
    };

    // Semantic search types for convenient access
    pub use crate::search::{
        CodeChunk, EmbeddingEngine, FileIndexer, IndexingOptions, IndexingStats, Language,
        SemanticConfig, SemanticSearcher, SemanticUtils, VectorStorage,
    };

    // Outline generation types for convenient access
    pub use crate::outline::{
        DiscoveredFile, FileDiscovery, FileDiscoveryConfig, FileDiscoveryReport, OutlineError,
    };

    // sah.toml configuration types removed (migrated to swissarmyhammer-config)
    // All TOML configuration functionality now provided by swissarmyhammer-config crate using figment

    // Common utilities for easy access
    pub use crate::common::{
        env_loader::EnvLoader,
        error_context::IoResultExt,
        file_types::{is_prompt_file, ExtensionMatcher},
        mcp_errors::{McpResultExt, ToSwissArmyHammerError},
        validation_builders::{quick, ValidationChain, ValidationErrorBuilder},
    };
}

/// Test utilities module for testing support
pub mod test_utils;

/// Test organization utilities for improved test management
pub mod test_organization;

/// Common utilities module for code reuse
pub mod common;
