//! Exit code constants for CLI commands
//!
//! These constants define the standard exit codes used throughout the application:
//! - 0: Success
//! - 1: General error or warnings
//! - 2: Validation errors or critical failures

/// Successful execution
pub const EXIT_SUCCESS: i32 = 0;

/// General error or warnings found
pub const EXIT_WARNING: i32 = 1;

/// Validation errors or critical failures
pub const EXIT_ERROR: i32 = 2;
