# swissarmyhammer-common

Common types and utilities shared across all SwissArmyHammer crates.

## Overview

This crate provides the foundational types, error handling, and traits used throughout the SwissArmyHammer ecosystem. It establishes consistent patterns for error classification, parameter handling, and common operations.

## Error Handling

### Severity Trait

All error types in SwissArmyHammer implement the `Severity` trait, which provides a standardized way to query error severity levels. This enables consistent error handling, appropriate logging levels, and better user-facing error presentation across the entire codebase.

#### Severity Levels

- **Critical**: System cannot continue, requires immediate attention
  - Examples: Repository not found, workflow execution failures, critical resource unavailable
  - Use when: The system encounters a problem that prevents continued operation or risks data integrity

- **Error**: Operation failed but system can continue
  - Examples: File not found, invalid format, permission denied for non-critical resource
  - Use when: A specific operation cannot complete but the system remains stable

- **Warning**: Potential issue but operation can proceed
  - Examples: Empty files, deprecation notices, rule violations
  - Use when: Issues that should be noted but don't prevent successful operation

#### Using the Severity Trait

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity, SwissArmyHammerError};

let error = SwissArmyHammerError::NotInGitRepository;

match error.severity() {
    ErrorSeverity::Critical => {
        eprintln!("Critical error: {}", error);
        std::process::exit(1);
    }
    ErrorSeverity::Error => {
        eprintln!("Error: {}", error);
        // Continue with fallback behavior
    }
    ErrorSeverity::Warning => {
        eprintln!("Warning: {}", error);
        // Log and continue normally
    }
}
```

#### Implementing Severity for Your Error Type

When creating a new error type in a SwissArmyHammer crate, implement the `Severity` trait to enable consistent error handling:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MyError {
    #[error("database corrupted")]
    DatabaseCorrupted,

    #[error("file not found: {path}")]
    FileNotFound { path: String },

    #[error("deprecated feature used")]
    DeprecatedFeature,
}

impl Severity for MyError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            MyError::DatabaseCorrupted => ErrorSeverity::Critical,
            MyError::FileNotFound { .. } => ErrorSeverity::Error,
            MyError::DeprecatedFeature => ErrorSeverity::Warning,
        }
    }
}
```

#### Severity Assignment Guidelines

When implementing `Severity` for your error types, follow these guidelines:

**Critical Errors** - System-level failures:
- Cannot continue operation
- Data loss possible
- Requires immediate attention
- Examples: Repository not found, workflow failures, storage backend errors

**Error Level** - Operation-specific failures:
- Specific operation fails
- System can continue
- No data loss
- Examples: File not found, parse errors, I/O errors

**Warning Level** - Non-critical issues:
- Operation can proceed
- Issue should be noted
- No immediate action required
- Examples: Rule violations, deprecation notices, empty files

### Error Context and Chaining

The crate provides utilities for adding context to errors and formatting error chains:

```rust
use swissarmyhammer_common::{ErrorContext, ErrorChainExt};

fn process_file(path: &str) -> Result<(), SwissArmyHammerError> {
    std::fs::read_to_string(path)
        .context(format!("Failed to read file: {}", path))?;
    Ok(())
}

// Format error chains for display
if let Err(e) = process_file("config.yaml") {
    eprintln!("{}", e.error_chain());
}
```

## SwissArmyHammerError

The `SwissArmyHammerError` enum provides common error types used across the ecosystem:

- **File Operations**: `FileNotFound`, `NotAFile`, `InvalidFilePath`, `PermissionDenied`
- **Repository**: `NotInGitRepository`, `DirectoryCreation`, `DirectoryAccess`
- **Workflow**: `WorkflowNotFound`, `WorkflowRunNotFound`
- **Serialization**: `Json`, `Serialization`
- **Storage**: `Storage`
- **Rules**: `RuleViolation`
- **General**: `Io`, `Context`, `Other`

## Features

This crate is designed to be lightweight and dependency-minimal, providing only the essential types needed across all SwissArmyHammer crates.

## Testing

Run tests with:
```bash
cargo test -p swissarmyhammer-common
```

## Documentation

Generate and view the full API documentation:
```bash
cargo doc -p swissarmyhammer-common --open
```

## License

See the main SwissArmyHammer repository for license information.
