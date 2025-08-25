//! Git operations and utilities
//!
//! This module provides both shell-based and git2-rs based git operations,
//! supporting a gradual migration from shell commands to native Rust git operations.

pub mod git2_utils;
pub mod operations;

#[cfg(test)]
mod integration_tests;

// Re-export main types for backward compatibility
pub use operations::GitOperations;
