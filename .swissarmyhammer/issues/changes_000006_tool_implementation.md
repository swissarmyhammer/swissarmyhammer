# Step 6: Implement McpTool Trait

Refer to ideas/changes.md

## Objective

Implement the `McpTool` trait for `GitChangesTool` with complete execute logic.

## Tasks

1. Create `GitChangesTool` struct and implement `McpTool` trait
   - `name()` returns `"git_changes"`
   - `description()` loads from tool descriptions registry
   - `schema()` returns JSON schema for GitChangesRequest
   - `execute()` implements the main logic

2. Implement `execute()` method:
   - Parse arguments into `GitChangesRequest`
   - Access git_ops from ToolContext
   - Validate git operations are available
   - Determine parent branch using `find_merge_target_for_issue()`
   - If parent exists: call `get_changed_files_from_parent()`
   - If no parent: call `get_all_tracked_files()`
   - Get uncommitted changes and merge with committed changes
   - Deduplicate file list
   - Build `GitChangesResponse`
   - Return success response

3. Error handling:
   - Git operations not available
   - Invalid branch name
   - Git operation failures
   - Convert GitError to McpError

## Success Criteria

- Tool compiles and implements McpTool trait
- Execute logic is complete and correct
- All error cases are handled
- Proper logging with tracing
- Code is well-documented

## Files to Modify

- `swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs`

## Estimated Code Changes

~120 lines