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
//! // Render a prompt
//! let mut context = swissarmyhammer::TemplateContext::new();
//! context.insert("language".to_string(), "rust".into());
//! context.insert("file".to_string(), "main.rs".into());
//! let rendered = library.render("code-review", &context)?;
//!
//! println!("{}", rendered);
//! # Ok(())
//! # }
//! ```

// Re-export core types

/// File source for loading prompts from various sources
pub use swissarmyhammer_common::file_loader::FileSource;

/// File system utilities and abstractions
pub use fs_utils::{FilePermissions, FileSystem, FileSystemUtils};

/// Plan command utilities
pub mod plan_utils;

/// Prompt filtering functionality
pub use swissarmyhammer_prompts::PromptFilter;

/// Advanced prompt loading and resolution
pub use swissarmyhammer_prompts::PromptResolver;

/// Backward compatibility alias for FileSource
pub use swissarmyhammer_common::file_loader::FileSource as PromptSource;

/// Core prompt management types and functionality
pub use swissarmyhammer_prompts::{Prompt, PromptLibrary, PromptLoader};

/// Template engine and rendering functionality (re-exported from swissarmyhammer-templating)
pub use swissarmyhammer_templating::{Template, TemplateEngine};

pub use swissarmyhammer_common::*;
pub use swissarmyhammer_workflow::{
    FileSystemWorkflowStorage, Workflow, WorkflowError, WorkflowExecutor, WorkflowName,
    WorkflowResolver, WorkflowRunId, WorkflowRunStatus, WorkflowStorage, WorkflowStorageBackend,
};

// sah.toml configuration types removed (migrated to swissarmyhammer-config)
// All TOML configuration functionality now provided by swissarmyhammer-config crate using figment

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
