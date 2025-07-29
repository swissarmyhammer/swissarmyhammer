# Update Merge Issue Tool for Worktrees

## Overview
Modify the `MergeIssueTool` to handle worktree cleanup when merging issues. The tool should detect if a worktree exists for the issue and clean it up after successful merge.

## Implementation

### Update MergeIssueTool (`src/mcp/tools/issues/merge/mod.rs`)

Update the execute method to use worktree-aware merge:

```rust
async fn execute(
    &self,
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> std::result::Result<CallToolResult, McpError> {
    let request: MergeIssueRequest = BaseToolImpl::parse_arguments(arguments)?;

    // Get the issue to determine its details
    let issue_storage = context.issue_storage.read().await;
    let issue = match issue_storage.get_issue(request.name.as_str()).await {
        Ok(issue) => issue,
        Err(e) => return Err(McpErrorHandler::handle_error(e, "get issue for merge")),
    };

    // Validate that the issue is completed before allowing merge
    if !issue.completed {
        return Ok(create_error_response(format!(
            "Issue '{}' must be completed before merging",
            request.name
        )));
    }

    // Merge branch and handle worktree cleanup
    let mut git_ops = context.git_ops.lock().await;
    let issue_name = issue.name.clone();

    match git_ops.as_mut() {
        Some(ops) => {
            // Check if worktree exists for this issue
            let worktree_path = ops.get_worktree_path(&issue_name);
            let has_worktree = worktree_path.exists() && 
                ops.worktree_exists(&worktree_path).unwrap_or(false);

            // Use appropriate merge method
            let merge_result = if has_worktree {
                ops.merge_issue_worktree(&issue_name, request.delete_branch)
            } else {
                // Fall back to branch-only merge for backward compatibility
                ops.merge_issue_branch(&issue_name)
                    .and_then(|_| {
                        if request.delete_branch {
                            let branch_name = format!("issue/{}", issue_name);
                            ops.delete_branch(&branch_name)?;
                        }
                        Ok(())
                    })
            };

            match merge_result {
                Ok(_) => {
                    let mut success_message = if has_worktree {
                        format!(
                            "Merged issue '{}' to main and cleaned up worktree",
                            issue_name
                        )
                    } else {
                        format!("Merged work branch for issue {} to main", issue_name)
                    };

                    // Get commit information after successful merge
                    let commit_info = match ops.get_last_commit_info() {
                        Ok(info) => {
                            let parts: Vec<&str> = info.split('|').collect();
                            if parts.len() >= 4 {
                                format!(
                                    "\n\nMerge commit: {}\nMessage: {}\nAuthor: {}\nDate: {}",
                                    &parts[0][..8], // First 8 chars of hash
                                    parts[1],
                                    parts[2],
                                    parts[3]
                                )
                            } else {
                                format!("\n\nMerge commit: {info}")
                            }
                        }
                        Err(_) => String::new(),
                    };

                    if request.delete_branch {
                        success_message.push_str(" and deleted branch");
                    }

                    success_message.push_str(&commit_info);
                    Ok(create_success_response(success_message))
                }
                Err(e) => {
                    tracing::error!("Merge failed for issue '{}': {}", issue_name, e);
                    Err(McpErrorHandler::handle_error(e, "merge issue"))
                }
            }
        }
        None => Ok(create_error_response(
            "Git operations not available".to_string(),
        )),
    }
}
```

### Add Cleanup Status Reporting

Add methods to report cleanup status:

```rust
impl MergeIssueTool {
    /// Build detailed merge status message
    fn build_merge_status(
        issue_name: &str,
        had_worktree: bool,
        branch_deleted: bool,
        commit_info: Option<String>,
    ) -> String {
        let mut parts = vec![
            format!("✅ Merged issue '{}'", issue_name),
        ];

        if had_worktree {
            parts.push("📁 Cleaned up worktree".to_string());
        }

        if branch_deleted {
            parts.push("🔀 Deleted issue branch".to_string());
        }

        let mut message = parts.join("\n");
        
        if let Some(info) = commit_info {
            message.push_str(&info);
        }

        message
    }
}
```

### Update Tool Description

Update the description file (`src/mcp/tools/issues/merge/description.md`):

```markdown
Merge the work branch for an issue back to the main branch. Automatically cleans up any associated worktree.

## Parameters

- `name` (required): Issue name to merge
- `delete_branch` (optional): Whether to delete the branch after merging (default: false)

## Examples

Merge an issue and keep the branch:
```json
{
  "name": "REFACTOR_000123_cleanup-code"
}
```

Merge an issue and delete the branch:
```json
{
  "name": "REFACTOR_000123_cleanup-code",
  "delete_branch": true
}
```

## Returns

Returns confirmation that:
- The issue branch has been merged to main
- The worktree has been cleaned up (if it existed)
- The branch has been deleted (if requested)
- Details of the merge commit

## Workflow

1. Validates the issue is marked as completed
2. Merges the `issue/<name>` branch to main
3. Removes the worktree at `.swissarmyhammer/worktrees/issue-<name>` if it exists
4. Optionally deletes the issue branch
5. Reports the merge commit details

This tool handles both worktree and branch-only workflows transparently.
```

## Dependencies
- Requires WORKTREE_000213 (merge worktree operation)

## Testing
1. Test merge with worktree cleanup
2. Test merge without worktree (backward compatibility)
3. Test branch deletion option
4. Test error handling for merge conflicts
5. Test commit info reporting

## Context
This step updates the merge tool to handle worktree cleanup automatically. It maintains backward compatibility for issues that were created before worktrees were introduced.