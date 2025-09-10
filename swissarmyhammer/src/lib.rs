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

#![warn(missing_docs)]

/// Storage abstractions and implementations
pub mod storage;

/// Plugin system for extensibility
pub mod plugins;

/// Security utilities for path validation and resource limits
pub mod security;

pub mod parameter_cli;

// Re-export core types

/// File source for loading prompts from various sources
pub use swissarmyhammer_common::file_loader::FileSource;

/// File system utilities and abstractions
pub use fs_utils::{FilePermissions, FileSystem, FileSystemUtils};

/// Plan command utilities
pub mod plan_utils;

/// Plugin system types for extending functionality
pub use plugins::{CustomLiquidFilter, PluginRegistry, SwissArmyHammerPlugin};

/// Prompt filtering functionality
pub use swissarmyhammer_prompts::PromptFilter;

/// Advanced prompt loading and resolution
pub use swissarmyhammer_prompts::PromptResolver;

/// Backward compatibility alias for FileSource
pub use swissarmyhammer_common::file_loader::FileSource as PromptSource;

/// Core prompt management types and functionality
pub use swissarmyhammer_prompts::{Prompt, PromptLibrary, PromptLoader};

/// Storage backends and abstractions
pub use storage::{PromptStorage, StorageBackend};

/// Template engine and rendering functionality (re-exported from swissarmyhammer-templating)
pub use swissarmyhammer_templating::{Template, TemplateEngine};

pub use swissarmyhammer_common::*;

// sah.toml configuration types removed (migrated to swissarmyhammer-config)
// All TOML configuration functionality now provided by swissarmyhammer-config crate using figment

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Test utilities module for testing support
pub mod test_utils;
