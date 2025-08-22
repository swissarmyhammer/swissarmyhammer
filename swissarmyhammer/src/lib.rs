//! # `SwissArmyHammer`
//!
//! A flexible prompt management library for AI assistants.
//!
//! ## Features
//!
//! - **Prompt Management**: Load, store, and organize prompts from various sources
//! - **Template Engine**: Powerful Liquid-based template processing
//! - **Search**: Full-text search capabilities for finding prompts
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

/// Template engine and rendering
pub mod template;

/// Storage abstractions and implementations
pub mod storage;

/// Prompt search functionality
pub mod prompt_search;

/// Semantic search functionality using vector embeddings
pub mod search;

/// Advanced search functionality
pub mod search_advanced;

/// Outline generation functionality for Tree-sitter based code analysis
pub mod outline;

/// Plugin system for extensibility
pub mod plugins;

/// Workflow system for state-based execution
pub mod workflow;

/// Shared frontmatter parsing functionality
pub mod frontmatter;

/// Issue tracking and management
pub mod issues;

/// Memoranda management and storage system
pub mod memoranda;

/// Todo list management system for ephemeral task tracking
pub mod todo;

/// Git operations for issue management
pub mod git;

/// Security utilities for path validation and resource limits
pub mod security;

/// Shell command security validation and control system
pub mod shell_security;

/// Advanced security hardening for shell command execution
pub mod shell_security_hardening;

/// Shell command performance monitoring and profiling
pub mod shell_performance;

/// File watching functionality for prompt directories
pub mod file_watcher;

/// Virtual file system for unified file loading
pub mod file_loader;

/// Directory traversal utilities
pub mod directory_utils;

/// Migration validation tools for SwissArmyHammer directory consolidation
pub mod migration;

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

/// Prompt filtering and search functionality
pub use prompt_filter::PromptFilter;

/// Advanced prompt loading and resolution
pub use prompt_resolver::PromptResolver;

/// Backward compatibility alias for FileSource
pub use file_loader::FileSource as PromptSource;

/// Core prompt management types and functionality
pub use prompts::{Prompt, PromptLibrary, PromptLoader};

/// Storage backends and abstractions
pub use storage::{PromptStorage, StorageBackend};

/// Template engine and rendering functionality
pub use template::{Template, TemplateEngine};

/// Workflow system for state-based execution
pub use workflow::{
    State, StateId, Transition, Workflow, WorkflowName, WorkflowRun, WorkflowRunId,
    WorkflowRunStatus,
};

/// Memoranda (memo/note) management types
pub use memoranda::{
    CreateMemoRequest, DeleteMemoRequest, GetMemoRequest, ListMemosResponse, Memo, MemoId,
    SearchMemosRequest, SearchMemosResponse, UpdateMemoRequest,
};

/// Todo list management types
pub use todo::{
    CreateTodoRequest, MarkCompleteTodoRequest, ShowTodoRequest, TodoId, TodoItem, TodoList,
    TodoStorage,
};

/// Migration validation types for directory consolidation
pub use migration::{
    scan_existing_directories, validate_migration_safety, ConflictInfo, ConflictSeverity,
    ConflictType, ContentSummary, GitRepositoryInfo, MigrationAction, MigrationPlan,
    MigrationScanResult,
};

/// Validation types and traits
pub use validation::{Validatable, ValidationIssue, ValidationLevel, ValidationResult};

/// sah.toml configuration types and functionality (new system)
pub use swissarmyhammer_config::{ConfigError as NewConfigError, ConfigProvider, TemplateContext};

/// Legacy sah.toml configuration compatibility layer
pub use swissarmyhammer_config::compat::{
    ConfigValue, Configuration, ConfigurationError, ValidationError,
};

/// New core data structures for sah.toml configuration  
pub use toml_core::{
    load_config as load_toml_core_config, load_repo_config as load_toml_core_repo_config,
    validate_config_file as validate_toml_core_config_file, ConfigError as TomlCoreError,
    ConfigParser as TomlCoreParser, ConfigValue as TomlCoreValue,
    Configuration as TomlCoreConfiguration,
};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Error types used throughout the library
pub mod error;

/// Configuration management
pub mod config;

/// Core TOML configuration data structures (new implementation)
pub mod toml_core;

pub use config::Config;
pub use error::{ErrorChainExt, ErrorContext, Result, SwissArmyHammerError};

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::{
        CustomLiquidFilter, FileSystem, FileSystemUtils, PluginRegistry, Prompt, PromptLibrary,
        PromptLoader, PromptStorage, Result, StorageBackend, SwissArmyHammerError,
        SwissArmyHammerPlugin, Template, TemplateEngine,
    };

    pub use crate::prompt_search::{SearchEngine, SearchResult};
    pub use crate::search_advanced::{
        generate_excerpt, AdvancedSearchEngine, AdvancedSearchOptions, AdvancedSearchResult,
    };
    pub use crate::workflow::{
        State, StateId, Transition, Workflow, WorkflowName, WorkflowRun, WorkflowRunId,
        WorkflowRunStatus,
    };

    // Memoranda types for convenient access
    pub use crate::memoranda::{
        CreateMemoRequest, DeleteMemoRequest, GetMemoRequest, ListMemosResponse, Memo, MemoId,
        SearchMemosRequest, SearchMemosResponse, UpdateMemoRequest,
    };

    // Todo types for convenient access
    pub use crate::todo::{
        CreateTodoRequest, MarkCompleteTodoRequest, ShowTodoRequest, TodoId, TodoItem, TodoList,
        TodoStorage,
    };

    // Migration validation types for convenient access
    pub use crate::migration::{
        scan_existing_directories, validate_migration_safety, ConflictInfo, ConflictSeverity,
        ConflictType, ContentSummary, GitRepositoryInfo, MigrationAction, MigrationPlan,
        MigrationScanResult,
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

    // sah.toml configuration types for convenient access (new system)
    pub use swissarmyhammer_config::{
        ConfigError as NewConfigError, ConfigProvider, TemplateContext,
    };

    // Legacy sah.toml configuration compatibility layer for convenient access
    pub use swissarmyhammer_config::compat::{
        load_and_merge_repo_config, load_config, load_repo_config, merge_config_into_context,
        validate_config_file, ConfigValue, Configuration, ConfigurationError, ValidationError,
    };

    // New core TOML configuration data structures for convenient access
    pub use crate::toml_core::{
        load_config as load_toml_core_config, load_repo_config as load_toml_core_repo_config,
        parse_config_file, parse_config_string,
        validate_config_file as validate_toml_core_config_file, ConfigError as TomlCoreError,
        ConfigParser as TomlCoreParser, ConfigValue as TomlCoreValue,
        Configuration as TomlCoreConfiguration,
    };

    // Common utilities for easy access
    pub use crate::common::{
        env_loader::EnvLoader,
        error_context::IoResultExt,
        file_types::{is_prompt_file, ExtensionMatcher},
        mcp_errors::{McpResultExt, ToSwissArmyHammerError},
        rate_limiter::{get_rate_limiter, RateLimitStatus, RateLimiter, RateLimiterConfig},
        validation_builders::{quick, ValidationChain, ValidationErrorBuilder},
    };
}

/// Test utilities module for testing support
pub mod test_utils;

/// Common utilities module for code reuse
pub mod common;
