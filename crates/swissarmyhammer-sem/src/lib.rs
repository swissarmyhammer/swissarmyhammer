//! Semantic diff engine — entity-level code diffing via tree-sitter.
//!
//! Harvested from the Ataraxy-Labs/sem project, with git2 dependency removed.
//! Provides semantic entity extraction, matching, and diffing for 12+ languages.

pub mod git_types;
pub mod model;
pub mod parser;
pub mod utils;
