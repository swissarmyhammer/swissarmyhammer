# Update Work Issue Tool for Worktrees

## Overview
Modify the `WorkIssueTool` to use worktrees instead of in-place branches. This involves updating the tool to call the new `create_work_worktree` method and adjusting the response messages.

## Implementation

### Update WorkIssueTool (`src/mcp/tools/issues/work/mod.rs`)

Replace the execute method implementation:

```rust
async fn execute(
    &self,
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> std::result::Result<CallToolResult, McpError> {
    let request: WorkIssueRequest = BaseToolImpl::parse_arguments(arguments)?;

    // Get the issue to determine its number for branch naming
    let issue_storage = context.issue_storage.read().await;
    let issue = match issue_storage.get_issue(request.name.as_str()).await {
        Ok(issue) => issue,
        Err(e) => return Err(McpErrorHandler::handle_error(e, "get issue")),
    };

    // Create work worktree
    let mut git_ops = context.git_ops.lock().await;
    let issue_name = issue.name.clone();

    match git_ops.as_mut() {
        Some(ops) => {
            match ops.create_work_worktree(&issue_name) {
                Ok(worktree_path) => {
                    let path_str = worktree_path.display().to_string();
                    Ok(create_success_response(format!(
                        "Created worktree for issue '{}' at: {}\n\nTo start working:\n  cd {}",
                        issue_name, path_str, path_str
                    )))
                }
                Err(e) => Err(McpErrorHandler::handle_error(e, "create work worktree")),
            }
        }
        None => Err(McpError::internal_error(
            "Git operations not available".to_string(),
            None,
        )),
    }
}
```

### Update Tool Description

Update the tool description file (`src/mcp/tools/issues/work/description.md`):

```markdown
Switch to a work worktree for the specified issue. Creates a dedicated workspace at `.swissarmyhammer/worktrees/issue-<issue_name>` if needed.

## Parameters

- `name` (required): Issue name to work on

## Examples

Start working on an issue:
```json
{
  "name": "REFACTOR_000123_cleanup-code"
}
```

## Returns

Returns the path to the worktree where you can work on the issue in isolation. If the worktree already exists, it will return the existing path.

## Workflow

1. Creates branch `issue/<issue_name>` if it doesn't exist
2. Creates a worktree at `.swissarmyhammer/worktrees/issue-<issue_name>`
3. Links the worktree to the issue branch
4. Returns the worktree path for you to navigate to

This provides complete isolation from the main repository, allowing you to work on multiple issues simultaneously without conflicts.
```

### Add Backward Compatibility Flag (Optional)

If needed, add a configuration option to use the old branch-based workflow:

```rust
impl WorkIssueTool {
    async fn execute_with_mode(
        &self,
        issue_name: &str,
        git_ops: &mut GitOperations,
        use_worktrees: bool,
    ) -> Result<String> {
        if use_worktrees {
            let path = git_ops.create_work_worktree(issue_name)?;
            Ok(format!("Created worktree at: {}", path.display()))
        } else {
            let branch = git_ops.create_work_branch(issue_name)?;
            Ok(format!("Switched to branch: {}", branch))
        }
    }
}
```

## Dependencies
- Requires WORKTREE_000211 (create worktree operation)

## Testing
1. Test creating worktree for new issue
2. Test resuming work on existing issue
3. Test error messages for invalid operations
4. Test response format is helpful for users

## Context
This step modifies the existing work tool to use worktrees. The change is transparent to users except for the response message which now shows the worktree path instead of just the branch name.