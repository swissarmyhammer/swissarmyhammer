//! DoctorRunner trait for implementing doctor commands
//!
//! Provides a common interface for doctor commands across different tools.

use crate::table::print_checks_table;
use crate::types::{Check, CheckStatus, ExitCode};

/// Trait for implementing doctor commands
///
/// Provides common functionality for accumulating checks and determining
/// exit codes. Implementors only need to provide storage for checks.
///
/// # Example
///
/// ```
/// use swissarmyhammer_doctor::{Check, CheckStatus, DoctorRunner};
///
/// struct MyDoctor {
///     checks: Vec<Check>,
/// }
///
/// impl DoctorRunner for MyDoctor {
///     fn checks(&self) -> &[Check] {
///         &self.checks
///     }
///
///     fn checks_mut(&mut self) -> &mut Vec<Check> {
///         &mut self.checks
///     }
/// }
///
/// let mut doctor = MyDoctor { checks: Vec::new() };
///
/// // Add a check
/// doctor.add_check(Check {
///     name: "Test".to_string(),
///     status: CheckStatus::Ok,
///     message: "Passed".to_string(),
///     fix: None,
/// });
///
/// // Get exit code
/// assert_eq!(doctor.get_exit_code(), 0);
/// ```
pub trait DoctorRunner {
    /// Get immutable reference to checks
    fn checks(&self) -> &[Check];

    /// Get mutable reference to checks for modification
    fn checks_mut(&mut self) -> &mut Vec<Check>;

    /// Add a check to the collection
    fn add_check(&mut self, check: Check) {
        self.checks_mut().push(check);
    }

    /// Add multiple checks at once
    fn add_checks(&mut self, checks: impl IntoIterator<Item = Check>) {
        self.checks_mut().extend(checks);
    }

    /// Get the exit code based on check results
    ///
    /// Returns:
    /// - 0: All checks passed (no errors or warnings)
    /// - 1: At least one warning detected
    /// - 2: At least one error detected
    fn get_exit_code(&self) -> i32 {
        let has_error = self.checks().iter().any(|c| c.status == CheckStatus::Error);
        let has_warning = self
            .checks()
            .iter()
            .any(|c| c.status == CheckStatus::Warning);

        let exit_code = if has_error {
            ExitCode::Error
        } else if has_warning {
            ExitCode::Warning
        } else {
            ExitCode::Success
        };

        exit_code.into()
    }

    /// Print the checks as a formatted table
    ///
    /// Uses comfy-table with colored status symbols.
    fn print_table(&self, verbose: bool) {
        print_checks_table(self.checks(), verbose);
    }

    /// Check if there are any errors
    fn has_errors(&self) -> bool {
        self.checks().iter().any(|c| c.status == CheckStatus::Error)
    }

    /// Check if there are any warnings
    fn has_warnings(&self) -> bool {
        self.checks()
            .iter()
            .any(|c| c.status == CheckStatus::Warning)
    }

    /// Get count of checks by status
    fn count_by_status(&self, status: CheckStatus) -> usize {
        self.checks().iter().filter(|c| c.status == status).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDoctor {
        checks: Vec<Check>,
    }

    impl TestDoctor {
        fn new() -> Self {
            Self { checks: Vec::new() }
        }
    }

    impl DoctorRunner for TestDoctor {
        fn checks(&self) -> &[Check] {
            &self.checks
        }

        fn checks_mut(&mut self) -> &mut Vec<Check> {
            &mut self.checks
        }
    }

    #[test]
    fn test_add_check() {
        let mut doctor = TestDoctor::new();
        assert_eq!(doctor.checks().len(), 0);

        doctor.add_check(Check {
            name: "Test".to_string(),
            status: CheckStatus::Ok,
            message: "Passed".to_string(),
            fix: None,
        });

        assert_eq!(doctor.checks().len(), 1);
        assert_eq!(doctor.checks()[0].name, "Test");
    }

    #[test]
    fn test_add_checks() {
        let mut doctor = TestDoctor::new();

        doctor.add_checks(vec![
            Check {
                name: "Test 1".to_string(),
                status: CheckStatus::Ok,
                message: "Passed".to_string(),
                fix: None,
            },
            Check {
                name: "Test 2".to_string(),
                status: CheckStatus::Warning,
                message: "Warning".to_string(),
                fix: Some("Fix it".to_string()),
            },
        ]);

        assert_eq!(doctor.checks().len(), 2);
    }

    #[test]
    fn test_get_exit_code_success() {
        let mut doctor = TestDoctor::new();
        doctor.add_check(Check {
            name: "Test".to_string(),
            status: CheckStatus::Ok,
            message: "Passed".to_string(),
            fix: None,
        });

        assert_eq!(doctor.get_exit_code(), 0);
    }

    #[test]
    fn test_get_exit_code_warning() {
        let mut doctor = TestDoctor::new();
        doctor.add_check(Check {
            name: "Test OK".to_string(),
            status: CheckStatus::Ok,
            message: "Passed".to_string(),
            fix: None,
        });
        doctor.add_check(Check {
            name: "Test Warning".to_string(),
            status: CheckStatus::Warning,
            message: "Warning".to_string(),
            fix: Some("Fix".to_string()),
        });

        assert_eq!(doctor.get_exit_code(), 1);
    }

    #[test]
    fn test_get_exit_code_error() {
        let mut doctor = TestDoctor::new();
        doctor.add_check(Check {
            name: "Test OK".to_string(),
            status: CheckStatus::Ok,
            message: "Passed".to_string(),
            fix: None,
        });
        doctor.add_check(Check {
            name: "Test Warning".to_string(),
            status: CheckStatus::Warning,
            message: "Warning".to_string(),
            fix: None,
        });
        doctor.add_check(Check {
            name: "Test Error".to_string(),
            status: CheckStatus::Error,
            message: "Error".to_string(),
            fix: Some("Fix it".to_string()),
        });

        // Error takes precedence over warning
        assert_eq!(doctor.get_exit_code(), 2);
    }

    #[test]
    fn test_has_errors() {
        let mut doctor = TestDoctor::new();
        assert!(!doctor.has_errors());

        doctor.add_check(Check {
            name: "Test".to_string(),
            status: CheckStatus::Warning,
            message: "Warning".to_string(),
            fix: None,
        });
        assert!(!doctor.has_errors());

        doctor.add_check(Check {
            name: "Test".to_string(),
            status: CheckStatus::Error,
            message: "Error".to_string(),
            fix: None,
        });
        assert!(doctor.has_errors());
    }

    #[test]
    fn test_has_warnings() {
        let mut doctor = TestDoctor::new();
        assert!(!doctor.has_warnings());

        doctor.add_check(Check {
            name: "Test".to_string(),
            status: CheckStatus::Ok,
            message: "OK".to_string(),
            fix: None,
        });
        assert!(!doctor.has_warnings());

        doctor.add_check(Check {
            name: "Test".to_string(),
            status: CheckStatus::Warning,
            message: "Warning".to_string(),
            fix: None,
        });
        assert!(doctor.has_warnings());
    }

    #[test]
    fn test_count_by_status() {
        let mut doctor = TestDoctor::new();

        doctor.add_checks(vec![
            Check {
                name: "OK 1".to_string(),
                status: CheckStatus::Ok,
                message: "OK".to_string(),
                fix: None,
            },
            Check {
                name: "OK 2".to_string(),
                status: CheckStatus::Ok,
                message: "OK".to_string(),
                fix: None,
            },
            Check {
                name: "Warning".to_string(),
                status: CheckStatus::Warning,
                message: "Warning".to_string(),
                fix: None,
            },
            Check {
                name: "Error".to_string(),
                status: CheckStatus::Error,
                message: "Error".to_string(),
                fix: None,
            },
        ]);

        assert_eq!(doctor.count_by_status(CheckStatus::Ok), 2);
        assert_eq!(doctor.count_by_status(CheckStatus::Warning), 1);
        assert_eq!(doctor.count_by_status(CheckStatus::Error), 1);
    }
}
