//! SwissArmyHammer Git Operations
//!
//! This crate provides a clean, type-safe interface for Git operations used throughout
//! the SwissArmyHammer project. It extracts git functionality from the main library
//! into a dedicated crate for better maintainability and reuse.
//!
//! ## Features
//!
//! - **Type Safety**: BranchName newtype prevents string confusion
//! - **Clean API**: Structured operations for repository, branches, commits
//! - **Better Errors**: Git-specific error types with proper context
//! - **Performance**: Direct git2 operations where possible
//! - **Testability**: Isolated git operations for easier testing
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use swissarmyhammer_git::{GitOperations, BranchName};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let git = GitOperations::new()?;
//! let branch = BranchName::new("feature/new-feature")?;
//! git.create_and_checkout_branch(&branch)?;
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod git2_utils;
pub mod operations;
pub mod repository;
pub mod types;

// Re-export main types
pub use error::{GitError, GitResult};
pub use operations::GitOperations;
pub use repository::GitRepository;
pub use types::{BranchName, CommitInfo, StatusSummary};

/// Version of this crate
pub const VERSION: &str = env!("CARGO_PKG_VERSION");