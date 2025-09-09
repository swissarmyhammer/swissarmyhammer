//! # Common Utility Functions
//!
//! This module provides utility functions and helper routines used throughout
//! the SwissArmyHammer ecosystem. These utilities are designed to be reusable
//! across different modules and crates.
//!
//! ## Categories
//!
//! - **String Processing**: Common string manipulation and validation utilities
//! - **Path Utilities**: File path processing and validation helpers
//! - **Serialization Helpers**: Utilities for advanced serialization scenarios
//! - **Validation Helpers**: Common validation functions and patterns
//! - **Error Utilities**: Error creation and context management helpers
//!
//! ## Design Principles
//!
//! - Functions should be pure and side-effect free where possible
//! - All functions should be well-documented with clear examples
//! - Functions should handle edge cases gracefully with proper error types
//! - Performance should be considered for frequently-used utilities
//!
//! ## Usage Notes
//!
//! Functions that are duplicated across multiple crates should be centralized here
//! to eliminate redundancy and ensure consistent behavior. New utilities should
//! follow the design principles and documentation standards outlined above.

pub mod directory_utils;
pub mod paths;
pub mod ulid_generator;

// Re-export commonly used ULID functions
pub use ulid_generator::{generate_monotonic_ulid, generate_monotonic_ulid_string};

// Re-export commonly used directory functions
pub use directory_utils::{
    find_git_repository_root, find_git_repository_root_from, find_swissarmyhammer_directory,
    find_swissarmyhammer_directory_from, get_or_create_swissarmyhammer_directory,
    get_or_create_swissarmyhammer_directory_from,
};
