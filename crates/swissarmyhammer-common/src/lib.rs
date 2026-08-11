//! # SwissArmyHammer Common
//!
//! This crate provides foundational types, traits, and utilities shared across
//! the SwissArmyHammer ecosystem. It serves as the base dependency for all other
//! SwissArmyHammer crates, establishing common patterns and abstractions.
//!
//! ## Modules
//!
//! - [`constants`] - Shared constants used throughout the ecosystem
//! - [`traits`] - Common trait definitions for shared behaviors
//! - [`types`] - Core type definitions and newtypes for domain safety
//! - [`utils`] - Utility functions and helpers
//!
//! ## Design Principles
//!
//! This crate follows the SwissArmyHammer architectural principles:
//! - Type safety through newtypes and strong typing
//! - Comprehensive error handling with structured error types
//! - Serialization support for all public types
//! - Documentation-driven development with clear API contracts

/// Build-time provenance — the git SHA baked into the binary.
pub mod build_info;
/// Shell command construction and failed-command reporting.
pub mod command;
/// Constants shared across the SwissArmyHammer ecosystem.
pub mod constants;
/// The SwissArmyHammer directory structure and its root resolution.
pub mod directory;
/// Open a file in the user's preferred editor.
pub mod editor;
/// Read environment variables with type conversion and defaults.
pub mod env_loader;
/// Error types shared across the SwissArmyHammer ecosystem.
pub mod error;
/// Helpers that attach context to errors.
pub mod error_context;
/// Virtual file system that loads files from a directory hierarchy.
pub mod file_loader;
/// File extension checks and file type detection.
pub mod file_types;
/// Split YAML frontmatter from a markdown body.
pub mod frontmatter;
/// File system abstraction with structured errors and test seams.
pub mod fs_utils;
/// Glob pattern expansion with gitignore support.
pub mod glob_utils;
/// The `Doctorable` health check framework.
pub mod health;
/// The `define_id!` macro that builds ULID newtypes.
pub mod id_types;
/// Interactive prompts that collect parameter values.
pub mod interactive_prompts;
/// Lenient JSONC reading for user-written configuration files.
pub mod json;
/// The `Initializable` lifecycle framework for components.
pub mod lifecycle;
/// Logging utilities shared by the SwissArmyHammer CLI crates.
pub mod logging;
/// Conversion of MCP failures into `SwissArmyHammerError`.
pub mod mcp_errors;
/// Conditions that make a parameter required or visible.
pub mod parameter_conditions;
/// The parameter system shared by prompts and workflows.
pub mod parameters;
/// Rules that decide which prompts become slash commands.
pub mod prompt_visibility;
/// Token bucket rate limiting for MCP operations.
pub mod rate_limiter;
/// Progress reporting for init and deinit lifecycle events.
pub mod reporter;
/// Canonical URL-safe slug generation.
pub mod slug;
/// Patterns that organize and group tests.
pub mod test_organization;
/// Test utilities — isolated temporary environments and process guards.
pub mod test_utils;
/// Trait definitions shared across the ecosystem.
pub mod traits;
/// Core type definitions and newtypes for domain safety.
pub mod types;
/// Thread-safe monotonic ULID generator.
pub mod ulid_generator;
/// Utility functions shared across the ecosystem.
pub mod utils;
/// The validation framework for content and workflow integrity.
pub mod validation;

// Re-export commonly used constants for convenience
pub use constants::DEFAULT_TEST_EMBEDDING_MODEL;
pub use parameters::*;
pub use test_utils::*;
pub use validation::*;

// Re-export commonly used ULID functions for convenience
pub use utils::{generate_monotonic_ulid, generate_monotonic_ulid_string};

// Re-export file_loader for convenience
pub use file_loader::{FileEntry, FileSource, SearchPath, VirtualFileSystem};

// Re-export commonly used directory functions for convenience
#[allow(deprecated)]
pub use utils::{
    find_git_repository_root_from, get_or_create_swissarmyhammer_directory,
    get_or_create_swissarmyhammer_directory_from,
};

// Re-export SwissarmyhammerDirectory for convenience
pub use directory::{DirectoryRootType, SwissarmyhammerDirectory};

// Re-export error types for convenience
pub use error::{ErrorSeverity, Result, Severity, SwissArmyHammerError};

// Re-export editor utility for convenience
pub use editor::open_in_editor;

// Re-export env_loader for convenience
pub use env_loader::EnvLoader;

// Re-export rate limiting functionality for convenience
pub use rate_limiter::{
    get_rate_limiter, init_rate_limiter, RateLimitChecker, RateLimitStatus, RateLimiter,
    RateLimiterConfig, DEFAULT_EXPENSIVE_OPERATION_LIMIT, DEFAULT_GLOBAL_RATE_LIMIT,
    DEFAULT_PER_CLIENT_RATE_LIMIT,
};

// Re-export the shared JSONC reader primitive for convenience
pub use json::{parse_jsonc, read_json_file, JsonFileError};

// Re-export glob utilities for convenience
pub use glob_utils::{
    expand_glob_patterns, matches_glob_pattern, parse_glob_pattern, validate_glob_pattern,
    GlobExpansionConfig, MAX_FILES,
};

// Re-export prompt visibility utilities for convenience
pub use prompt_visibility::{is_prompt_partial, is_prompt_visible};

// Re-export reporter types for convenience
pub use reporter::{CliReporter, InitEvent, InitReporter, NullReporter, TracingReporter};

// Re-export the canonical slug function — mirrored by
// `kanban-app/ui/src/lib/slugify.ts` and kept in lockstep via the parity
// corpus in `swissarmyhammer-common/tests/slug_parity_corpus.txt`.
pub use slug::slug;

// Re-export logging utilities for convenience
pub use logging::FileWriterGuard;

// Re-export test utilities for convenience (when testing)
pub use test_utils::{acquire_semantic_db_lock, create_temp_dir, ProcessGuard};

// Pretty wrapper for formatting types as YAML in logs
use serde::Serialize;
use std::fmt::{self, Debug, Formatter};

/// Wrapper for pretty-printing types in logs as YAML
/// Use in tracing statements: info!("Config: {}", Pretty(&config));
pub struct Pretty<T>(pub T);

/// Render a value as YAML, and fall back to pretty `Debug` when the value
/// cannot serialize. Both [`Pretty`] format impls call this, so the two
/// renderings cannot drift apart.
fn format_pretty<T: Serialize + Debug>(obj: &T, f: &mut Formatter<'_>) -> fmt::Result {
    match serde_yaml_ng::to_string(obj) {
        Ok(yaml) => write!(f, "\n{}", yaml),
        Err(_) => write!(f, "\n{:#?}", obj),
    }
}

impl<T: Serialize + Debug> fmt::Display for Pretty<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        format_pretty(&self.0, f)
    }
}

impl<T: Serialize + Debug> fmt::Debug for Pretty<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        format_pretty(&self.0, f)
    }
}

pub use error::*;

#[cfg(test)]
mod pretty_tests {
    use super::Pretty;
    use serde::Serialize;

    /// A normal serializable struct for testing the happy path.
    #[derive(Debug, Serialize)]
    struct Config {
        name: String,
        count: u32,
    }

    /// A struct whose Serialize impl always fails, used to exercise the
    /// fallback path that renders via Debug instead of YAML.
    #[derive(Debug)]
    struct Unserializable {
        _label: String,
    }

    impl Serialize for Unserializable {
        fn serialize<S: serde::Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("intentional failure"))
        }
    }

    #[test]
    fn display_renders_yaml_for_serializable_type() {
        let val = Config {
            name: "hello".into(),
            count: 42,
        };
        let output = format!("{}", Pretty(&val));
        // YAML output should contain key-value pairs
        assert!(
            output.contains("name: hello"),
            "expected YAML key 'name', got: {output}"
        );
        assert!(
            output.contains("count: 42"),
            "expected YAML key 'count', got: {output}"
        );
        // Output should start with a newline (the format uses "\n{yaml}")
        assert!(
            output.starts_with('\n'),
            "expected leading newline, got: {output}"
        );
    }

    #[test]
    fn debug_renders_yaml_for_serializable_type() {
        let val = Config {
            name: "world".into(),
            count: 7,
        };
        let output = format!("{:?}", Pretty(&val));
        assert!(
            output.contains("name: world"),
            "expected YAML key 'name', got: {output}"
        );
        assert!(
            output.contains("count: 7"),
            "expected YAML key 'count', got: {output}"
        );
        assert!(
            output.starts_with('\n'),
            "expected leading newline, got: {output}"
        );
    }

    #[test]
    fn display_falls_back_to_debug_when_serialize_fails() {
        let val = Unserializable {
            _label: "fallback".into(),
        };
        let output = format!("{}", Pretty(&val));
        // Fallback uses {:#?} (pretty Debug), so it should contain the struct name and field
        assert!(
            output.contains("Unserializable"),
            "expected Debug struct name, got: {output}"
        );
        assert!(
            output.contains("fallback"),
            "expected field value, got: {output}"
        );
        assert!(
            output.starts_with('\n'),
            "expected leading newline, got: {output}"
        );
    }

    #[test]
    fn debug_falls_back_to_debug_when_serialize_fails() {
        let val = Unserializable {
            _label: "debug_fallback".into(),
        };
        let output = format!("{:?}", Pretty(&val));
        assert!(
            output.contains("Unserializable"),
            "expected Debug struct name, got: {output}"
        );
        assert!(
            output.contains("debug_fallback"),
            "expected field value, got: {output}"
        );
        assert!(
            output.starts_with('\n'),
            "expected leading newline, got: {output}"
        );
    }
}
