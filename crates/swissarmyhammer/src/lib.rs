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
//! use swissarmyhammer::TemplateLibrary;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a new prompt library
//! let mut library = TemplateLibrary::new();
//!
//! // Add prompts from a directory
//! if std::path::Path::new("./.prompts").exists() {
//!     library.add_directory("./.prompts")?;
//! }
//!
//! // Render a prompt
//! let mut context = swissarmyhammer::TemplateContext::new();
//! context.set("language".to_string(), serde_json::json!("rust"));
//! context.set("file".to_string(), serde_json::json!("main.rs"));
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

/// Advanced template loading and resolution
pub use swissarmyhammer_templating::PromptResolver;

/// Core template management types and functionality
pub use swissarmyhammer_templating::{Prompt, TemplateLibrary, TemplateLoader};

/// Template engine and rendering functionality (re-exported from swissarmyhammer-templating)
pub use swissarmyhammer_templating::{Template, TemplateEngine};

/// Template context for prompt rendering
pub use swissarmyhammer_config::TemplateContext;

pub use swissarmyhammer_common::*;

// sah.toml configuration types removed (migrated to swissarmyhammer-config)
// All TOML configuration functionality now provided by swissarmyhammer-config crate using figment

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
