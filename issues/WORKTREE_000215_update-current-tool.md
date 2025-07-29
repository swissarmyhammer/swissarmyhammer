# Update Current Issue Tool for Worktrees

## Overview
Modify the `CurrentIssueTool` to detect active worktrees in addition to the current branch. This enables users to see which issue they're working on regardless of whether they're using branches or worktrees.

## Implementation

### Update CurrentIssueTool (`src/mcp/tools/issues/current/mod.rs`)

Replace the execute method implementation:

```rust
async fn execute(
    &self,
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> std::result::Result<CallToolResult, McpError> {
    let _request: CurrentIssueRequest = BaseToolImpl::parse_arguments(arguments)?;

    let git_ops = context.git_ops.lock().await;
    match git_ops.as_ref() {
        Some(ops) => {
            // First check for active worktrees
            match ops.list_issue_worktrees() {
                Ok(worktrees) if !worktrees.is_empty() => {
                    // Report all active worktrees
                    let mut message = String::from("Active issue worktrees:\n");
                    for wt in worktrees {
                        message.push_str(&format!(
                            "- {} at {}\n",
                            wt.issue_name,
                            wt.path.display()
                        ));
                    }
                    
                    // Also check current branch
                    if let Ok(current_issue) = ops.get_current_issue() {
                        if let Some(issue) = current_issue {
                            message.push_str(&format!("\nCurrent context: working on issue '{}'", issue));
                        }
                    }
                    
                    Ok(create_success_response(message))
                }
                _ => {
                    // Fall back to branch-based detection
                    match ops.current_branch() {
                        Ok(branch) => {
                            let config = Config::global();
                            if let Some(issue_name) = branch.strip_prefix(&config.issue_branch_prefix) {
                                Ok(create_success_response(format!(
                                    "Currently working on issue: {} (branch mode)",
                                    issue_name
                                )))
                            } else {
                                Ok(create_success_response(format!(
                                    "Not on an issue branch. Current branch: {}",
                                    branch
                                )))
                            }
                        }
                        Err(e) => Err(McpErrorHandler::handle_error(e, "get current branch")),
                    }
                }
            }
        }
        None => Ok(create_error_response(
            "Git operations not available".to_string(),
        )),
    }
}
```

### Enhanced Detection Logic

Add a method to provide more detailed current issue information:

```rust
impl CurrentIssueTool {
    /// Get detailed information about the current issue context
    fn get_issue_context(ops: &GitOperations) -> Result<IssueContext> {
        let mut context = IssueContext::default();
        
        // Check worktrees
        if let Ok(worktrees) = ops.list_issue_worktrees() {
            context.worktrees = worktrees;
        }
        
        // Check current branch
        if let Ok(branch) = ops.current_branch() {
            context.current_branch = Some(branch.clone());
            let config = Config::global();
            if let Some(issue_name) = branch.strip_prefix(&config.issue_branch_prefix) {
                context.current_issue_branch = Some(issue_name.to_string());
            }
        }
        
        // Determine primary working context
        if let Ok(Some(issue)) = ops.get_current_issue() {
            context.primary_issue = Some(issue);
        }
        
        Ok(context)
    }
}

#[derive(Default)]
struct IssueContext {
    worktrees: Vec<IssueWorktree>,
    current_branch: Option<String>,
    current_issue_branch: Option<String>,
    primary_issue: Option<String>,
}
```

### Update Tool Description

Update the description file (`src/mcp/tools/issues/current/description.md`):

```markdown
Get the current issue being worked on. Detects both worktree and branch-based workflows.

## Parameters

- `branch` (optional): Which branch to check (optional, defaults to current)

## Examples

Check current issue:
```json
{}
```

## Returns

Returns information about:
- Active issue worktrees and their locations
- Current branch if it's an issue branch
- The primary issue context you're working in

## Detection Priority

1. If you're inside a worktree, that issue is reported as current
2. If you're on an issue branch, that issue is reported
3. All active worktrees are listed for reference

This tool helps you understand your current working context across multiple active issues.
```

## Dependencies
- Requires WORKTREE_000212 (worktree status detection)

## Testing
1. Test detection in worktree
2. Test detection on issue branch
3. Test with multiple active worktrees
4. Test when not working on any issue
5. Test output format clarity

## Context
This step updates the current tool to be aware of both worktree and branch-based workflows, providing comprehensive information about the user's working context.