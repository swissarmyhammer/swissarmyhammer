//! Integration tests for public exports

use swissarmyhammer_common::{ErrorSeverity, Severity};

#[test]
fn test_error_severity_export() {
    // Verify ErrorSeverity is accessible from crate root
    let _warning = ErrorSeverity::Warning;
    let _error = ErrorSeverity::Error;
    let _critical = ErrorSeverity::Critical;
}

#[test]
fn test_severity_trait_export() {
    // Verify Severity trait is accessible from crate root
    #[derive(Debug)]
    enum TestError {
        Minor,
    }

    impl Severity for TestError {
        fn severity(&self) -> ErrorSeverity {
            ErrorSeverity::Warning
        }
    }

    let error = TestError::Minor;
    assert_eq!(error.severity(), ErrorSeverity::Warning);
}
