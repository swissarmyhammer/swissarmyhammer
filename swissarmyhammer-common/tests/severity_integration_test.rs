//! Integration tests for Severity trait implementation
//!
//! This test suite validates that the Severity trait is correctly implemented
//! and that all severity levels function as expected across different error types.
//!
//! ## Test Scope
//!
//! These tests are "integration" tests in the sense that they test the Severity
//! trait integration across all SwissArmyHammerError variants, ensuring complete
//! and consistent coverage of the trait implementation. They validate the trait
//! works correctly across the entire error type hierarchy within swissarmyhammer-common.
//!
//! These are not cross-crate integration tests (which would test error handling
//! across different workspace crates). Each crate has its own unit tests for
//! Severity trait implementations.

use std::path::PathBuf;
use swissarmyhammer_common::{ErrorSeverity, Severity, SwissArmyHammerError};

#[test]
fn test_severity_trait_basics() {
    // Test that severity levels are distinct
    assert_ne!(ErrorSeverity::Warning, ErrorSeverity::Error);
    assert_ne!(ErrorSeverity::Error, ErrorSeverity::Critical);
    assert_ne!(ErrorSeverity::Warning, ErrorSeverity::Critical);
}

#[test]
fn test_severity_trait_equality() {
    // Test that severity levels can be compared
    assert_eq!(ErrorSeverity::Warning, ErrorSeverity::Warning);
    assert_eq!(ErrorSeverity::Error, ErrorSeverity::Error);
    assert_eq!(ErrorSeverity::Critical, ErrorSeverity::Critical);
}

#[test]
fn test_swissarmyhammer_error_critical_severity() {
    // Test that all critical errors are properly classified
    let critical_errors = vec![
        SwissArmyHammerError::NotInGitRepository,
        SwissArmyHammerError::DirectoryCreation("test error".to_string()),
        SwissArmyHammerError::DirectoryAccess("test error".to_string()),
        SwissArmyHammerError::WorkflowNotFound("workflow-name".to_string()),
        SwissArmyHammerError::WorkflowRunNotFound("run-id".to_string()),
        SwissArmyHammerError::Storage("storage error".to_string()),
        SwissArmyHammerError::PermissionDenied {
            path: "/test/path".to_string(),
            error: "access denied".to_string(),
            suggestion: "check permissions".to_string(),
        },
    ];

    for error in critical_errors {
        assert_eq!(
            error.severity(),
            ErrorSeverity::Critical,
            "Expected Critical severity for: {}",
            error
        );
    }
}

#[test]
fn test_swissarmyhammer_error_error_severity() {
    // Test that all error-level errors are properly classified

    // Create a serialization error by trying to parse invalid YAML
    let yaml_error: serde_yaml::Error =
        serde_yaml::from_str::<serde_yaml::Value>("invalid: yaml: content:").unwrap_err();

    // Create a JSON error by trying to parse invalid JSON
    let json_error: serde_json::Error =
        serde_json::from_str::<serde_json::Value>("{invalid json").unwrap_err();

    let error_level_errors: Vec<SwissArmyHammerError> = vec![
        SwissArmyHammerError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "test io error",
        )),
        SwissArmyHammerError::Serialization(yaml_error),
        SwissArmyHammerError::Json(json_error),
        SwissArmyHammerError::FileNotFound {
            path: "/test/file.txt".to_string(),
            suggestion: "check path".to_string(),
        },
        SwissArmyHammerError::NotAFile {
            path: "/test/dir".to_string(),
            suggestion: "use a file path".to_string(),
        },
        SwissArmyHammerError::InvalidFilePath {
            path: "invalid:path".to_string(),
            suggestion: "use valid path".to_string(),
        },
        SwissArmyHammerError::InvalidPath {
            path: PathBuf::from("/invalid/path"),
        },
        SwissArmyHammerError::IoContext {
            message: "io context error".to_string(),
        },
        SwissArmyHammerError::Semantic {
            message: "semantic error".to_string(),
        },
        SwissArmyHammerError::Other {
            message: "other error".to_string(),
        },
    ];

    for error in error_level_errors {
        assert_eq!(
            error.severity(),
            ErrorSeverity::Error,
            "Expected Error severity for: {}",
            error
        );
    }
}

#[test]
fn test_swissarmyhammer_error_warning_severity() {
    // Test that all warning-level errors are properly classified
    let warning = SwissArmyHammerError::RuleViolation("rule broken".to_string());

    assert_eq!(
        warning.severity(),
        ErrorSeverity::Warning,
        "Expected Warning severity for: {}",
        warning
    );
}

#[test]
fn test_custom_error_type_with_severity() {
    // Test that custom error types can implement Severity trait
    #[derive(Debug)]
    enum CustomError {
        Fatal,
        Recoverable,
        Minor,
    }

    impl Severity for CustomError {
        fn severity(&self) -> ErrorSeverity {
            match self {
                CustomError::Fatal => ErrorSeverity::Critical,
                CustomError::Recoverable => ErrorSeverity::Error,
                CustomError::Minor => ErrorSeverity::Warning,
            }
        }
    }

    let fatal = CustomError::Fatal;
    let recoverable = CustomError::Recoverable;
    let minor = CustomError::Minor;

    assert_eq!(fatal.severity(), ErrorSeverity::Critical);
    assert_eq!(recoverable.severity(), ErrorSeverity::Error);
    assert_eq!(minor.severity(), ErrorSeverity::Warning);
}

#[test]
fn test_severity_usage_pattern() {
    // Test a realistic usage pattern of checking severity
    let error = SwissArmyHammerError::NotInGitRepository;

    match error.severity() {
        ErrorSeverity::Critical => {
            // This is the expected path - no assertion needed
        }
        ErrorSeverity::Error => {
            panic!("Expected Critical severity");
        }
        ErrorSeverity::Warning => {
            panic!("Expected Critical severity");
        }
    }
}

#[test]
fn test_all_error_variants_have_severity() {
    // This test ensures we don't forget to add severity for new variants
    // By testing at least one example of each variant category

    use std::io;

    let test_cases = vec![
        // Critical
        (
            SwissArmyHammerError::NotInGitRepository,
            ErrorSeverity::Critical,
        ),
        (
            SwissArmyHammerError::DirectoryCreation("test".to_string()),
            ErrorSeverity::Critical,
        ),
        (
            SwissArmyHammerError::DirectoryAccess("test".to_string()),
            ErrorSeverity::Critical,
        ),
        (
            SwissArmyHammerError::WorkflowNotFound("test".to_string()),
            ErrorSeverity::Critical,
        ),
        (
            SwissArmyHammerError::WorkflowRunNotFound("test".to_string()),
            ErrorSeverity::Critical,
        ),
        (
            SwissArmyHammerError::Storage("test".to_string()),
            ErrorSeverity::Critical,
        ),
        (
            SwissArmyHammerError::PermissionDenied {
                path: "test".to_string(),
                error: "test".to_string(),
                suggestion: "test".to_string(),
            },
            ErrorSeverity::Critical,
        ),
        // Error
        (
            SwissArmyHammerError::Io(io::Error::new(io::ErrorKind::Other, "test")),
            ErrorSeverity::Error,
        ),
        (
            SwissArmyHammerError::Serialization(
                serde_yaml::from_str::<serde_yaml::Value>("invalid: yaml: content:").unwrap_err(),
            ),
            ErrorSeverity::Error,
        ),
        (
            SwissArmyHammerError::Json(
                serde_json::from_str::<serde_json::Value>("{invalid json").unwrap_err(),
            ),
            ErrorSeverity::Error,
        ),
        (
            SwissArmyHammerError::FileNotFound {
                path: "test".to_string(),
                suggestion: "test".to_string(),
            },
            ErrorSeverity::Error,
        ),
        (
            SwissArmyHammerError::NotAFile {
                path: "test".to_string(),
                suggestion: "test".to_string(),
            },
            ErrorSeverity::Error,
        ),
        (
            SwissArmyHammerError::InvalidFilePath {
                path: "test".to_string(),
                suggestion: "test".to_string(),
            },
            ErrorSeverity::Error,
        ),
        (
            SwissArmyHammerError::InvalidPath {
                path: PathBuf::from("test"),
            },
            ErrorSeverity::Error,
        ),
        (
            SwissArmyHammerError::IoContext {
                message: "test".to_string(),
            },
            ErrorSeverity::Error,
        ),
        (
            SwissArmyHammerError::Semantic {
                message: "test".to_string(),
            },
            ErrorSeverity::Error,
        ),
        (
            SwissArmyHammerError::Other {
                message: "test".to_string(),
            },
            ErrorSeverity::Error,
        ),
        // Warning
        (
            SwissArmyHammerError::RuleViolation("test".to_string()),
            ErrorSeverity::Warning,
        ),
    ];

    for (error, expected_severity) in test_cases {
        assert_eq!(
            error.severity(),
            expected_severity,
            "Severity mismatch for: {}",
            error
        );
    }
}
