//! Core type definitions for the doctor module

/// Status of a diagnostic check
#[derive(Debug, PartialEq, Clone)]
pub enum CheckStatus {
    /// Check passed without issues
    Ok,
    /// Check passed but with potential issues
    Warning,
    /// Check failed with errors
    Error,
}

/// Exit codes for the doctor command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    /// All checks passed
    Success = 0,
    /// Warnings detected
    Warning = 1,
    /// Errors detected
    Error = 2,
}

impl From<ExitCode> for i32 {
    fn from(code: ExitCode) -> i32 {
        code as i32
    }
}

/// Result of a single diagnostic check
#[derive(Debug, Clone)]
pub struct Check {
    /// Name of the check performed
    pub name: String,
    /// Status of the check (Ok, Warning, Error)
    pub status: CheckStatus,
    /// Descriptive message about the check result
    pub message: String,
    /// Optional fix suggestion for warnings or errors
    pub fix: Option<String>,
}

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

    #[test]
    fn test_check_clone() {
        let check = Check {
            name: "Test".to_string(),
            status: CheckStatus::Ok,
            message: "Message".to_string(),
            fix: Some("Fix".to_string()),
        };
        let cloned = check.clone();
        assert_eq!(cloned.name, "Test");
        assert_eq!(cloned.status, CheckStatus::Ok);
        assert_eq!(cloned.message, "Message");
        assert_eq!(cloned.fix, Some("Fix".to_string()));
    }
}
