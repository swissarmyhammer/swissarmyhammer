# PLAN_000006: File Path Validation

**Refer to ./specification/plan.md**

## Goal

Implement comprehensive file path validation for the plan command, including existence checks, permission validation, and support for both relative and absolute paths with clear error messages.

## Background

The plan command needs robust file validation to provide a good user experience. Users should get clear, helpful error messages when they specify invalid files, and the system should handle various path formats correctly.

## Requirements

1. Validate file existence before workflow execution
2. Check file readability permissions
3. Support both relative and absolute paths
4. Handle special cases (directories, symlinks, etc.)
5. Provide clear, actionable error messages
6. Follow existing error handling patterns in the codebase
7. Add proper error types if needed

## Implementation Details

### Validation Logic

```rust
fn validate_plan_file(plan_filename: &str) -> Result<std::path::PathBuf, PlanError> {
    let path = std::path::Path::new(plan_filename);
    
    // Check if path exists
    if !path.exists() {
        return Err(PlanError::FileNotFound {
            path: plan_filename.to_string(),
            suggestion: "Check the file path and ensure the file exists".to_string(),
        });
    }
    
    // Check if it's a file (not directory)
    if !path.is_file() {
        return Err(PlanError::NotAFile {
            path: plan_filename.to_string(),
            suggestion: "Path must point to a markdown file, not a directory".to_string(),
        });
    }
    
    // Check readability
    match std::fs::File::open(path) {
        Ok(_) => Ok(path.to_path_buf()),
        Err(e) => Err(PlanError::PermissionDenied {
            path: plan_filename.to_string(),
            error: e.to_string(),
            suggestion: "Check file permissions and ensure you can read the file".to_string(),
        }),
    }
}
```

### Error Types

Define appropriate error types (or use existing ones):

```rust
#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    #[error("Plan file not found: {path}\nSuggestion: {suggestion}")]
    FileNotFound { path: String, suggestion: String },
    
    #[error("Path is not a file: {path}\nSuggestion: {suggestion}")]
    NotAFile { path: String, suggestion: String },
    
    #[error("Permission denied accessing file: {path}\nError: {error}\nSuggestion: {suggestion}")]
    PermissionDenied { path: String, error: String, suggestion: String },
    
    #[error("Invalid file format: {path}\nSuggestion: {suggestion}")]
    InvalidFormat { path: String, suggestion: String },
}
```

### Integration with Command Handler

Update the command handler from PLAN_000005:

```rust
Commands::Plan { plan_filename } => {
    // Validate the plan file
    let validated_path = match validate_plan_file(&plan_filename) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error: {}", e);
            return Err(e.into());
        }
    };
    
    // Use the validated path for execution
    let vars = vec![
        ("plan_filename".to_string(), validated_path.to_string_lossy().to_string())
    ];
    
    execute_workflow("plan", vars, Vec::new(), false, false, false, None, false).await?;
}
```

## Validation Features

### 1. Path Format Support
- Absolute paths: `/full/path/to/plan.md`
- Relative paths: `./specification/plan.md`, `plans/feature.md`
- Home directory expansion if needed: `~/plans/plan.md`

### 2. File Type Validation
- Ensure path points to a file, not directory
- Optional: Check for markdown file extension
- Handle symlinks appropriately

### 3. Permission Checks
- Verify file is readable
- Provide helpful error messages for permission issues
- Handle edge cases gracefully

### 4. Error Message Quality
- Clear description of what went wrong
- Actionable suggestions for fixing the problem
- Context about what was expected

## Implementation Steps

1. Research existing error types in the codebase
2. Define or reuse appropriate error types for plan validation
3. Implement file validation function
4. Add comprehensive path handling (relative/absolute)
5. Implement permission checking
6. Create helpful error messages
7. Integrate validation into command handler
8. Add unit tests for validation logic
9. Test with various file scenarios

## Acceptance Criteria

- [ ] File existence validation works correctly
- [ ] Directory vs file detection implemented
- [ ] Permission checking works properly
- [ ] Both relative and absolute paths supported
- [ ] Clear error messages for all failure cases
- [ ] Helpful suggestions included in error messages
- [ ] Integration with command handler complete
- [ ] Comprehensive test coverage

## Testing Scenarios

- Valid markdown file (should pass)
- Non-existent file (should error with clear message)
- Directory instead of file (should error appropriately)
- File without read permissions (should error with suggestion)
- Relative path that exists
- Absolute path that exists
- Empty filename or invalid characters

## Dependencies

- Requires command handler from PLAN_000005
- Should integrate with existing error handling system
- May need to define new error types

## Notes

- Follow existing error handling patterns in the codebase
- Use `std::path::Path` for proper path handling
- Consider using `thiserror` or existing error handling approach
- Provide consistent error message formatting
- Consider logging validation attempts for debugging