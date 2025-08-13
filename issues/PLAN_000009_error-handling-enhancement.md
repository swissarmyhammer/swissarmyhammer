# PLAN_000009: Error Handling Enhancement

**Refer to ./specification/plan.md**

## Goal

Enhance error handling throughout the plan command implementation to provide comprehensive, user-friendly error messages and proper error propagation following the existing patterns in the swissarmyhammer codebase.

## Background

Building on the basic file validation from PLAN_000006, we need to implement comprehensive error handling that covers all potential failure scenarios in the plan command workflow, from CLI parsing through workflow execution to issue creation.

## Requirements

1. Define comprehensive error types for plan command failures
2. Implement user-friendly error messages with actionable suggestions
3. Handle workflow execution errors gracefully
4. Add proper error logging and debugging support
5. Follow existing error handling patterns in the codebase
6. Ensure error messages are consistent with CLI standards
7. Add recovery suggestions where possible

## Implementation Details

### Comprehensive Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum PlanCommandError {
    #[error("Plan file not found: {path}")]
    FileNotFound {
        path: String,
        #[source]
        source: std::io::Error,
    },
    
    #[error("Permission denied accessing plan file: {path}")]
    PermissionDenied {
        path: String,
        #[source]  
        source: std::io::Error,
    },
    
    #[error("Invalid plan file format: {path}\nReason: {reason}")]
    InvalidFileFormat {
        path: String,
        reason: String,
    },
    
    #[error("Workflow execution failed for plan: {plan_filename}")]
    WorkflowExecutionFailed {
        plan_filename: String,
        #[source]
        source: WorkflowError,
    },
    
    #[error("Issue creation failed during planning")]
    IssueCreationFailed {
        plan_filename: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    
    #[error("Plan file is empty or contains no valid content: {path}")]
    EmptyPlanFile {
        path: String,
    },
    
    #[error("Plan file too large to process: {path} ({size} bytes)")]
    FileTooLarge {
        path: String,
        size: u64,
    },
    
    #[error("Issues directory is not writable")]
    IssuesDirectoryNotWritable {
        path: String,
        #[source]
        source: std::io::Error,
    },
}
```

### Error Context and User Guidance

```rust
impl PlanCommandError {
    /// Provide user-friendly guidance for resolving the error
    pub fn user_guidance(&self) -> String {
        match self {
            PlanCommandError::FileNotFound { path, .. } => {
                format!(
                    "The plan file '{}' was not found.\n\
                    \n\
                    Suggestions:\n\
                    • Check the file path for typos\n\
                    • Ensure the file exists: ls -la '{}'\n\
                    • Try using an absolute path: swissarmyhammer plan /full/path/to/{}\n\
                    • Create the file if it doesn't exist",
                    path, path, path
                )
            }
            PlanCommandError::PermissionDenied { path, .. } => {
                format!(
                    "Permission denied when trying to read '{}'.\n\
                    \n\
                    Suggestions:\n\
                    • Check file permissions: ls -la '{}'\n\
                    • Ensure you have read access: chmod +r '{}'\n\
                    • Try running with appropriate permissions",
                    path, path, path
                )
            }
            PlanCommandError::InvalidFileFormat { path, reason } => {
                format!(
                    "The plan file '{}' has an invalid format.\n\
                    Reason: {}\n\
                    \n\
                    Suggestions:\n\
                    • Ensure the file is a valid markdown file\n\
                    • Check for proper UTF-8 encoding\n\
                    • Verify the file isn't corrupted",
                    path, reason
                )
            }
            PlanCommandError::WorkflowExecutionFailed { plan_filename, .. } => {
                format!(
                    "Failed to execute planning workflow for '{}'.\n\
                    \n\
                    Suggestions:\n\
                    • Check that the plan file contains valid content\n\
                    • Ensure the issues directory is writable\n\
                    • Try running with --debug for more details\n\
                    • Check system resources and permissions",
                    plan_filename
                )
            }
            PlanCommandError::EmptyPlanFile { path } => {
                format!(
                    "The plan file '{}' is empty or contains no valid content.\n\
                    \n\
                    Suggestions:\n\
                    • Add content to the plan file\n\
                    • Ensure the file isn't just whitespace\n\
                    • Check that the file saved properly",
                    path
                )
            }
            PlanCommandError::FileTooLarge { path, size } => {
                format!(
                    "The plan file '{}' is too large ({} bytes).\n\
                    \n\
                    Suggestions:\n\
                    • Break large plans into smaller, focused files\n\
                    • Remove unnecessary content from the plan\n\
                    • Consider splitting into multiple planning sessions",
                    path, size
                )
            }
            PlanCommandError::IssuesDirectoryNotWritable { path, .. } => {
                format!(
                    "Cannot write to issues directory: '{}'.\n\
                    \n\
                    Suggestions:\n\
                    • Check directory permissions: ls -la '{}'\n\
                    • Ensure you have write access: chmod +w '{}'\n\
                    • Create the directory if it doesn't exist: mkdir -p '{}'",
                    path, path, path, path
                )
            }
            PlanCommandError::IssueCreationFailed { plan_filename, .. } => {
                format!(
                    "Failed to create issue files for plan '{}'.\n\
                    \n\
                    Suggestions:\n\
                    • Ensure the issues directory exists and is writable\n\
                    • Check available disk space\n\
                    • Verify no conflicting files exist\n\
                    • Try running with --debug for more details",
                    plan_filename
                )
            }
        }
    }
    
    /// Get the error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            PlanCommandError::FileNotFound { .. } => ErrorSeverity::Error,
            PlanCommandError::PermissionDenied { .. } => ErrorSeverity::Error,
            PlanCommandError::InvalidFileFormat { .. } => ErrorSeverity::Error,
            PlanCommandError::WorkflowExecutionFailed { .. } => ErrorSeverity::Critical,
            PlanCommandError::IssueCreationFailed { .. } => ErrorSeverity::Critical,
            PlanCommandError::EmptyPlanFile { .. } => ErrorSeverity::Warning,
            PlanCommandError::FileTooLarge { .. } => ErrorSeverity::Error,
            PlanCommandError::IssuesDirectoryNotWritable { .. } => ErrorSeverity::Error,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ErrorSeverity {
    Warning,
    Error,
    Critical,
}
```

### Enhanced File Validation

```rust
fn validate_plan_file_comprehensive(plan_filename: &str) -> Result<ValidatedPlanFile, PlanCommandError> {
    let path = std::path::Path::new(plan_filename);
    
    // Check file existence
    if !path.exists() {
        return Err(PlanCommandError::FileNotFound {
            path: plan_filename.to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "File not found"),
        });
    }
    
    // Check if it's a file
    if !path.is_file() {
        return Err(PlanCommandError::InvalidFileFormat {
            path: plan_filename.to_string(),
            reason: "Path points to a directory, not a file".to_string(),
        });
    }
    
    // Check file size
    let metadata = path.metadata().map_err(|e| PlanCommandError::PermissionDenied {
        path: plan_filename.to_string(),
        source: e,
    })?;
    
    const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10MB
    if metadata.len() > MAX_FILE_SIZE {
        return Err(PlanCommandError::FileTooLarge {
            path: plan_filename.to_string(),
            size: metadata.len(),
        });
    }
    
    // Check if file is empty
    if metadata.len() == 0 {
        return Err(PlanCommandError::EmptyPlanFile {
            path: plan_filename.to_string(),
        });
    }
    
    // Check readability
    let content = std::fs::read_to_string(path).map_err(|e| {
        match e.kind() {
            std::io::ErrorKind::PermissionDenied => PlanCommandError::PermissionDenied {
                path: plan_filename.to_string(),
                source: e,
            },
            _ => PlanCommandError::InvalidFileFormat {
                path: plan_filename.to_string(),
                reason: format!("Cannot read file: {}", e),
            },
        }
    })?;
    
    // Basic content validation
    if content.trim().is_empty() {
        return Err(PlanCommandError::EmptyPlanFile {
            path: plan_filename.to_string(),
        });
    }
    
    // Check UTF-8 validity (already done by read_to_string, but explicit check)
    if content.contains('\0') {
        return Err(PlanCommandError::InvalidFileFormat {
            path: plan_filename.to_string(),
            reason: "File contains null bytes - may be binary".to_string(),
        });
    }
    
    Ok(ValidatedPlanFile {
        path: path.to_path_buf(),
        content,
        size: metadata.len(),
    })
}

#[derive(Debug)]
struct ValidatedPlanFile {
    path: std::path::PathBuf,
    content: String,
    size: u64,
}
```

### Error Display and Logging

```rust
impl PlanCommandError {
    /// Display error with appropriate formatting for CLI
    pub fn display_to_user(&self, use_color: bool) -> String {
        let error_prefix = if use_color {
            "\x1b[31mError:\x1b[0m" // Red "Error:"
        } else {
            "Error:"
        };
        
        let guidance = self.user_guidance();
        
        format!("{} {}\n\n{}", error_prefix, self, guidance)
    }
    
    /// Log error with appropriate level
    pub fn log_error(&self) {
        match self.severity() {
            ErrorSeverity::Warning => log::warn!("{}", self),
            ErrorSeverity::Error => log::error!("{}", self),
            ErrorSeverity::Critical => log::error!("CRITICAL: {}", self),
        }
        
        // Log source chain for debugging
        let mut source = self.source();
        while let Some(err) = source {
            log::debug!("Caused by: {}", err);
            source = err.source();
        }
    }
}
```

## Implementation Steps

1. Research existing error handling patterns in swissarmyhammer codebase
2. Define comprehensive error types following existing conventions
3. Implement enhanced file validation with detailed error scenarios
4. Add user guidance and suggestion system
5. Implement error display formatting with color support
6. Add comprehensive error logging
7. Update command handler to use enhanced error handling
8. Update integration tests to verify error scenarios
9. Add error handling documentation
10. Test all error scenarios thoroughly

## Acceptance Criteria

- [ ] Comprehensive error types defined for all failure scenarios
- [ ] User-friendly error messages with actionable suggestions
- [ ] Proper error propagation and source chain handling
- [ ] Enhanced file validation with detailed checks
- [ ] Color-coded error display support
- [ ] Appropriate error logging at different levels
- [ ] Integration with existing error handling patterns
- [ ] Comprehensive test coverage of error scenarios
- [ ] Documentation of error handling approach

## Testing Strategy

- Test each error scenario individually
- Verify error messages are helpful and actionable
- Test error propagation through the call chain
- Validate logging output at different levels
- Test color formatting and plain text modes
- Verify error recovery suggestions actually work

## Dependencies

- Requires file validation from PLAN_000006
- Builds on command handler from PLAN_000005
- Should integrate with existing error types in codebase
- May require updates to integration tests from PLAN_000008

## Notes

- Follow existing error handling conventions in the codebase
- Use `thiserror` or the established error handling approach
- Ensure errors are both machine-readable and human-friendly
- Consider internationalization for error messages if needed
- Test error scenarios as thoroughly as success scenarios
- Error messages should guide users toward solutions, not just report problems