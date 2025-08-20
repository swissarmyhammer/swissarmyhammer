# CLI MCP Integration: Refactor Issue Commands

## Overview

Refactor the CLI issue commands in `swissarmyhammer-cli/src/issue.rs` to use MCP tools directly instead of duplicating business logic, eliminating the "horrific blunder" mentioned in the specification.

## Problem Statement

The `issue.rs` module contains ~474 lines of duplicated business logic that mirrors functionality already implemented in MCP tools:

- `create_issue()` duplicates `CreateIssueTool`
- `complete_issue()` duplicates `MarkCompleteTool` 
- `work_issue()` duplicates `WorkIssueTool`
- `merge_issue()` duplicates `MergeIssueTool`
- `show_current_issue()` duplicates `CurrentIssueTool`
- `show_next_issue()` duplicates `NextIssueTool`
- And several other functions

## Goals

1. Replace all issue command implementations with direct MCP tool calls
2. Maintain identical CLI behavior and output formatting
3. Reduce code duplication and maintenance burden
4. Ensure comprehensive test coverage for refactored commands

## MCP Tools Mapping

| CLI Function | MCP Tool | Tool Arguments |
|-------------|----------|----------------|
| `create_issue()` | `issue_create` | `name`, `content` |
| `list_issues()` | Custom logic | Multiple tools + formatting |
| `show_issue()` | Custom logic | List + filter by name |
| `update_issue()` | `issue_update` | `name`, `content`, `append` |
| `complete_issue()` | `issue_mark_complete` | `name` |
| `work_issue()` | `issue_work` | `name` |
| `merge_issue()` | `issue_merge` | `name`, `delete_branch` |
| `show_current_issue()` | `issue_current` | `branch` (optional) |
| `show_status()` | `issue_all_complete` | None |
| `show_next_issue()` | `issue_next` | None |

## Tasks

### 1. Refactor Individual Command Functions

Transform each function from direct business logic to MCP tool calls:

**Before (create_issue example):**
```rust
async fn create_issue(
    storage: FileSystemIssueStorage,
    name: Option<String>,
    content: Option<String>,
    file: Option<std::path::PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = get_content_from_args(content, file)?;
    let issue_name = name.unwrap_or(NAMELESS_ISSUE_NAME.to_string());
    let issue = storage.create_issue(issue_name, content).await?;
    // ... formatting logic
}
```

**After:**
```rust
async fn create_issue(
    context: &CliToolContext,
    name: Option<String>,
    content: Option<String>,
    file: Option<std::path::PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = get_content_from_args(content, file)?;
    let issue_name = name.unwrap_or(NAMELESS_ISSUE_NAME.to_string());
    
    let args = context.create_arguments(vec![
        ("name", json!(issue_name)),
        ("content", json!(content)),
    ]);
    
    let result = context.execute_tool("issue_create", args).await?;
    println!("{}", response_formatting::format_success_response(&result));
    Ok(())
}
```

### 2. Update Main Handler Function

Replace storage instantiation with tool context:

```rust
pub async fn handle_issue_command(
    command: IssueCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = CliToolContext::new().await?;

    match command {
        IssueCommands::Create { name, content, file } => {
            create_issue(&context, name, content, file).await?;
        }
        // ... other commands using context
    }
    Ok(())
}
```

### 3. Handle Complex Operations

Some CLI functions combine multiple operations or require special formatting:

- **`list_issues()`**: May need to call multiple MCP tools and combine results
- **`show_issue()`**: Needs to list all issues then filter by name  
- **`print_issues_table()`**: Custom formatting logic that should be preserved

These functions should:
1. Make appropriate MCP tool calls
2. Apply CLI-specific formatting to the results
3. Maintain existing behavior exactly

### 4. Preserve CLI-Specific Logic

Keep the following CLI-specific functions:
- `get_content_from_args()` - Handles stdin and file input
- `format_issue_status()` - CLI-specific formatting
- Various printing and formatting functions

### 5. Update Tests

Update existing tests in `swissarmyhammer-cli/tests/` to work with the new implementation:
- Mock `CliToolContext` for unit tests
- Add integration tests that verify MCP tool calls
- Ensure all CLI behaviors are preserved

## Implementation Approach

### Phase 1: Simple Commands
Start with simple one-to-one mappings:
- `complete_issue()` → `issue_mark_complete`
- `work_issue()` → `issue_work` 
- `show_current_issue()` → `issue_current`
- `show_next_issue()` → `issue_next`

### Phase 2: Complex Commands  
Handle commands requiring multiple MCP calls or special logic:
- `create_issue()`
- `merge_issue()`
- `update_issue()`

### Phase 3: Composite Commands
Handle commands that combine multiple operations:
- `list_issues()`
- `show_issue()`
- `show_status()`

## Acceptance Criteria

- [ ] All issue commands use MCP tools instead of direct storage access
- [ ] CLI behavior and output formatting remain identical
- [ ] All existing tests pass with new implementation
- [ ] Code reduction: `issue.rs` should be <200 lines (down from ~474)
- [ ] No direct use of `FileSystemIssueStorage` in CLI commands
- [ ] Integration tests verify MCP tool execution
- [ ] Error handling maintains existing user experience

## Risk Mitigation

1. **Behavioral Changes**: Comprehensive before/after testing of all commands
2. **Error Handling**: Ensure MCP error messages are user-friendly in CLI context
3. **Performance**: Monitor any performance impact from additional abstraction layer

## Expected Changes

- Modified: `swissarmyhammer-cli/src/issue.rs` (~274 lines removed, ~50 lines modified)
- Modified: `swissarmyhammer-cli/tests/issue_*.rs` (test updates)
- New: Integration tests for MCP tool calls from CLI

## Dependencies

- Requires: CLI_000220_project-setup (CliToolContext implementation)
- Requires: All issue MCP tools to be stable and tested

## Follow-up Issues

Success here enables similar refactoring of `memo.rs` and `search.rs` modules.

## Proposed Solution

After analyzing the current `issue.rs` implementation (474 lines) and the `CliToolContext` integration layer, I will implement the refactoring in phases:

### Implementation Strategy

**Phase 1: Simple 1:1 Command Mappings**
- `complete_issue()` → `issue_mark_complete` MCP tool
- `work_issue()` → `issue_work` MCP tool  
- `show_current_issue()` → `issue_current` MCP tool
- `show_next_issue()` → `issue_next` MCP tool

**Phase 2: Complex Commands**
- `create_issue()` → `issue_create` MCP tool + CLI formatting
- `update_issue()` → `issue_update` MCP tool + CLI formatting
- `merge_issue()` → `issue_merge` MCP tool + CLI formatting

**Phase 3: Composite Commands**
- `list_issues()` → Multiple MCP tools + existing CLI formatting logic
- `show_issue()` → List + filter + existing CLI formatting
- `show_status()` → `issue_all_complete` MCP tool + CLI formatting

### Key Implementation Decisions

1. **Keep CLI-Specific Functions**: Preserve `get_content_from_args()`, `format_issue_status()`, and printing functions
2. **Use CliToolContext**: Replace `FileSystemIssueStorage` with `CliToolContext` and MCP tool calls
3. **Maintain Identical Output**: Ensure all CLI behavior and formatting remains exactly the same
4. **Error Handling**: Map MCP errors to user-friendly CLI messages using `response_formatting` utilities

### Expected Outcome
- Reduce code from ~474 lines to <200 lines
- Eliminate duplicate business logic
- Maintain identical CLI user experience
- Enable easier maintenance through single source of truth