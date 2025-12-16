//! # SwissArmyHammer Modes Domain Crate
//!
//! This crate provides mode management functionality for SwissArmyHammer,
//! including loading mode definitions with system prompts for different agent types.
//!
//! ## Mode File Format
//!
//! Modes are defined in markdown files with YAML frontmatter:
//!
//! ```markdown
//! ---
//! name: general-purpose
//! description: General-purpose agent for researching complex questions
//! ---
//!
//! You are a general-purpose AI agent capable of researching complex
//! questions, searching for code, and executing multi-step tasks.
//!
//! Your approach should be thorough and methodical...
//! ```
//!
//! ## Features
//!
//! - **Mode Management**: Load and organize modes from various sources
//! - **Frontmatter Parsing**: Extract mode metadata from YAML frontmatter
//! - **System Prompts**: Each mode has a dedicated system prompt
//! - **Discovery**: Hierarchical mode loading from builtin and user directories

#![warn(missing_docs)]

mod frontmatter;
mod mode;
mod registry;

pub use frontmatter::{parse_frontmatter, FrontmatterResult};
pub use mode::Mode;
pub use registry::ModeRegistry;

use swissarmyhammer_common::SwissArmyHammerError;

/// Result type for mode operations
pub type Result<T> = std::result::Result<T, SwissArmyHammerError>;

// Include builtin modes using include_dir
use include_dir::{include_dir, Dir};

/// Builtin modes directory embedded at compile time
static BUILTIN_MODES_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../builtin/modes");

/// Get builtin mode files as (id, content) tuples
pub fn builtin_modes() -> Vec<(&'static str, &'static str)> {
    BUILTIN_MODES_DIR
        .files()
        .filter_map(|file| {
            let path = file.path();
            if path.extension()? == "md" {
                let id = path.file_stem()?.to_str()?;
                let content = file.contents_utf8()?;
                Some((id, content))
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_creation() {
        let mode = Mode::new(
            "test-mode",
            "Test Mode",
            "A test mode for unit tests",
            "You are a test agent.",
        );

        assert_eq!(mode.id(), "test-mode");
        assert_eq!(mode.name(), "Test Mode");
        assert_eq!(mode.description(), "A test mode for unit tests");
        assert_eq!(mode.system_prompt(), "You are a test agent.");
    }
}
