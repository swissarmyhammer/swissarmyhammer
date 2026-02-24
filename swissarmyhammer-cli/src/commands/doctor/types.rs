//! Type definitions for the doctor module
//!
//! Re-exports core types from swissarmyhammer-doctor.

// Re-export core doctor types from the shared crate
pub use swissarmyhammer_doctor::{Check, CheckStatus, ExitCode};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_status_equality() {
        assert_eq!(CheckStatus::Ok, CheckStatus::Ok);
        assert_ne!(CheckStatus::Ok, CheckStatus::Warning);
        assert_ne!(CheckStatus::Warning, CheckStatus::Error);
    }

    #[test]
    fn test_exit_code_conversion() {
        assert_eq!(i32::from(ExitCode::Success), 0);
        assert_eq!(i32::from(ExitCode::Warning), 1);
        assert_eq!(i32::from(ExitCode::Error), 2);
    }

    #[test]
    fn test_exit_code_equality() {
        assert_eq!(ExitCode::Success, ExitCode::Success);
        assert_ne!(ExitCode::Success, ExitCode::Warning);
    }
}
