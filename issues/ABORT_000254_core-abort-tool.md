# ABORT_000254: Core Abort Tool Implementation

Refer to ./specification/abort.md

## Objective

Implement the core functionality of the abort MCP tool that creates `.swissarmyhammer/.abort` files with the abort reason. This establishes the file-based abort system that replaces string-based detection.

## Context

Building on the project setup from ABORT_000253, this step implements the actual file creation logic for the abort tool. The specification calls for atomic file operations that create a `.swissarmyhammer/.abort` file containing the abort reason as plain text.

## Tasks

### 1. Implement File Creation Logic

**Update: `swissarmyhammer-tools/src/mcp/tools/abort/create/mod.rs`**

```rust
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use crate::mcp::responses::ToolResponse;
use crate::mcp::error_handling::McpError;

pub async fn abort_create_tool(
    args: HashMap<String, Value>
) -> Result<ToolResponse, McpError> {
    // Extract required reason parameter
    let reason = args.get("reason")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::InvalidParameters {
            message: "Missing required parameter 'reason'".to_string()
        })?;

    // Ensure .swissarmyhammer directory exists
    let sah_dir = Path::new(".swissarmyhammer");
    if !sah_dir.exists() {
        fs::create_dir_all(sah_dir)
            .map_err(|e| McpError::IoError {
                message: format!("Failed to create .swissarmyhammer directory: {}", e),
                source: e,
            })?;
    }

    // Create abort file with reason
    let abort_file_path = sah_dir.join(".abort");
    fs::write(&abort_file_path, reason)
        .map_err(|e| McpError::IoError {
            message: format!("Failed to create abort file: {}", e),
            source: e,
        })?;

    tracing::info!("Abort file created with reason: {}", reason);

    Ok(ToolResponse::success(format!(
        "Abort signal created successfully. Reason: {}",
        reason
    )))
}
```

### 2. Add Parameter Validation Tests

**Create: `swissarmyhammer-tools/src/mcp/tools/abort/create/tests.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_abort_tool_creates_file() {
        let _temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(_temp_dir.path()).unwrap();

        let mut args = HashMap::new();
        args.insert("reason".to_string(), json!("Test abort reason"));

        let result = abort_create_tool(args).await;
        assert!(result.is_ok());

        let abort_path = Path::new(".swissarmyhammer/.abort");
        assert!(abort_path.exists());
        
        let contents = fs::read_to_string(abort_path).unwrap();
        assert_eq!(contents, "Test abort reason");
    }

    #[tokio::test]
    async fn test_abort_tool_missing_reason() {
        let args = HashMap::new();
        let result = abort_create_tool(args).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_abort_tool_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let mut args = HashMap::new();
        args.insert("reason".to_string(), json!("Directory creation test"));

        let result = abort_create_tool(args).await;
        assert!(result.is_ok());

        assert!(Path::new(".swissarmyhammer").exists());
        assert!(Path::new(".swissarmyhammer/.abort").exists());
    }
}
```

**Update: `swissarmyhammer-tools/src/mcp/tools/abort/create/mod.rs`**
Add test module:
```rust
#[cfg(test)]
mod tests;
```

### 3. Add Proper Error Handling Types

**Ensure: `swissarmyhammer-tools/src/mcp/error_handling.rs`**
Verify IoError variant exists:
```rust
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("Invalid parameters: {message}")]
    InvalidParameters { message: String },
    
    #[error("IO error: {message}")]
    IoError { 
        message: String,
        #[source]
        source: std::io::Error 
    },
    // ... other variants
}
```

### 4. Update Tool Description with Implementation Details

**Update: `swissarmyhammer-tools/src/mcp/tools/abort/create/description.md`**
```markdown
Create an abort file to signal immediate termination of workflows and prompts.

## Parameters

- `reason` (required): String containing the abort reason/message

## Behavior

- Creates `.swissarmyhammer/.abort` file with the reason text
- Creates `.swissarmyhammer/` directory if it doesn't exist
- Atomic file operation ensures abort state persists across process boundaries
- File-based approach is more robust than string-based detection

## Examples

Create an abort signal with reason:
```json
{
  "reason": "User cancelled the destructive operation"
}
```

Abort due to validation failure:
```json
{
  "reason": "Pre-condition validation failed - cannot proceed safely"
}
```

## Returns

Returns success confirmation with the abort reason. The abort file will cause any running workflows to terminate immediately when they check for the abort condition.

## File Location

The abort file is created at `.swissarmyhammer/.abort` in the current working directory.

## Error Conditions

- Missing or invalid `reason` parameter
- Unable to create `.swissarmyhammer/` directory
- Unable to write abort file (permissions, disk space, etc.)
```

### 5. Integration Test

**Create: `swissarmyhammer-tools/tests/abort_tool_integration_test.rs`**

```rust
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use serde_json::json;
use swissarmyhammer_tools::mcp::tools::abort::create::abort_create_tool;
use std::collections::HashMap;

#[tokio::test]
async fn test_abort_tool_end_to_end() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let mut args = HashMap::new();
    args.insert("reason".to_string(), json!("Integration test abort"));

    let result = abort_create_tool(args).await;
    assert!(result.is_ok());

    let abort_file = Path::new(".swissarmyhammer/.abort");
    assert!(abort_file.exists());
    
    let content = fs::read_to_string(abort_file).unwrap();
    assert_eq!(content, "Integration test abort");

    std::env::set_current_dir(original_dir).unwrap();
}
```

## Success Criteria

- [ ] Abort tool creates `.swissarmyhammer/.abort` file with reason text
- [ ] Tool handles missing `.swissarmyhammer/` directory creation
- [ ] Proper parameter validation with clear error messages
- [ ] Atomic file operations ensure reliability
- [ ] Comprehensive unit tests covering success and error cases
- [ ] Integration test verifies end-to-end functionality
- [ ] All tests pass with `cargo test`
- [ ] Tool can be invoked through MCP interface

## Testing

```bash
# Run specific abort tool tests
cargo test abort_tool

# Run integration tests
cargo test abort_tool_integration

# Verify no regressions
cargo test

# Check compilation
cargo check
```

## Notes

- File creation is atomic to prevent race conditions
- Uses `tracing::info!` for proper logging integration
- Follows established error handling patterns from other MCP tools
- Directory creation uses `create_dir_all` for robustness
- Tests use `tempfile` crate for isolated test environments

## Next Steps

After completion, proceed to ABORT_000255 for WorkflowRun cleanup integration.