# Working Directory and Environment Variable Support

Refer to /Users/wballard/github/sah-shell/ideas/shell.md

## Overview

Implement comprehensive working directory resolution and environment variable management for shell command execution, with proper validation and security controls.

## Objective

Add support for configurable working directories and environment variable injection while maintaining security and following established patterns from the existing codebase.

## Requirements

### Working Directory Support
- Accept optional working directory parameter
- Resolve relative paths safely
- Validate directory existence and accessibility
- Default to current working directory if not specified
- Integrate with existing directory utilities

### Environment Variable Management
- Support additional environment variables via HashMap
- Merge with existing process environment
- Handle environment variable validation and sanitization
- Support common development environment patterns

### Path Resolution and Validation
- Use existing `directory_utils.rs` patterns
- Validate paths against directory traversal attacks
- Ensure working directory exists and is accessible
- Handle permission errors gracefully

### Response Metadata
- Include resolved working directory in response
- Log environment variable changes (without sensitive values)
- Provide clear error messages for path/permission issues
- Maintain audit trail for security

## Implementation Details

### Working Directory Implementation
```rust
use std::path::{Path, PathBuf};
use std::env;

fn resolve_working_directory(
    working_directory: Option<String>
) -> Result<PathBuf, ShellError> {
    match working_directory {
        Some(dir) => {
            let path = Path::new(&dir);
            if path.is_relative() {
                env::current_dir()?.join(path)
            } else {
                path.to_path_buf()
            }
        },
        None => env::current_dir().map_err(ShellError::from),
    }
}
```

### Environment Variable Handling
```rust
use std::collections::HashMap;

fn setup_environment(
    additional_env: Option<HashMap<String, String>>
) -> HashMap<String, String> {
    let mut env_vars = env::vars().collect::<HashMap<_, _>>();
    
    if let Some(additional) = additional_env {
        for (key, value) in additional {
            // Validate and sanitize environment variables
            if validate_env_var(&key, &value) {
                env_vars.insert(key, value);
            }
        }
    }
    
    env_vars
}
```

### Command Builder Integration
```rust
use std::process::Command;

fn build_command(
    command_str: &str,
    working_dir: &Path,
    env_vars: &HashMap<String, String>
) -> Command {
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
       .arg(command_str)
       .current_dir(working_dir)
       .envs(env_vars);
    
    cmd
}
```

## Security Considerations

### Path Validation
- Prevent directory traversal attacks (`../`, `..\\`)
- Validate against absolute paths outside allowed areas
- Check directory permissions before execution
- Log all working directory changes for audit

### Environment Variable Security
- Sanitize environment variable names and values
- Prevent injection of malicious environment variables
- Filter sensitive variables from logs
- Validate environment variable formats

### Permission Handling
- Handle permission denied errors gracefully
- Provide informative error messages
- Respect system-level access controls
- Fail safely on permission issues

## Integration Points

### Existing Directory Utils
- Leverage `swissarmyhammer/src/directory_utils.rs` patterns
- Use existing path validation functions
- Follow established security patterns
- Maintain consistency with other tools

### Configuration Integration
- Support configuration-based directory restrictions (future)
- Integrate with existing configuration system
- Allow per-execution overrides
- Maintain backward compatibility

## Acceptance Criteria

- [ ] Working directory parameter works correctly
- [ ] Relative and absolute paths handled properly
- [ ] Environment variables merged correctly
- [ ] Path validation prevents security issues
- [ ] Permission errors handled gracefully
- [ ] Response metadata includes resolved paths
- [ ] Integration with existing directory utils
- [ ] Cross-platform path handling works

## Testing Requirements

- [ ] Unit tests for path resolution logic
- [ ] Tests for environment variable merging
- [ ] Security tests for directory traversal prevention
- [ ] Permission error handling tests
- [ ] Cross-platform compatibility tests
- [ ] Integration tests with various working directories

## Notes

- Build on the timeout and process management from the previous step
- Focus on security and proper validation of user inputs
- Ensure compatibility with existing directory utility patterns
- Working directory support is essential for build/development workflows
- Environment variable support enables complex development scenarios