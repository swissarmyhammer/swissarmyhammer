//! Display objects for doctor command output
//!
//! Re-exports display types from swissarmyhammer-doctor for external use.

// Re-export display types from the shared crate (for external consumers)
#[allow(unused_imports)]
pub use swissarmyhammer_doctor::{
    categorize_check, format_check_status, CheckResult, VerboseCheckResult,
};
