# Create swissarmyhammer-issues Crate for Issue Management

## Problem

Issue management functionality is currently part of the main `swissarmyhammer` crate, preventing `swissarmyhammer-tools` from being independent. The tools crate extensively uses:

- `swissarmyhammer::issues::{FileSystemIssueStorage, IssueStorage, Issue, IssueInfo, IssueName}`

## Solution

Create a new `swissarmyhammer-issues` crate that contains all issue management functionality.

## Components to Extract

- `IssueStorage` trait and implementations
- `FileSystemIssueStorage` implementation
- `Issue`, `IssueInfo`, `IssueName` types
- Issue creation, completion, merging logic
- Issue file format handling (markdown files in `./issues/` directory)
- Issue status tracking and branch management

## Files Currently Using Issue Functionality

- `swissarmyhammer-tools/src/test_utils.rs`
- `swissarmyhammer-tools/src/mcp/server.rs`
- `swissarmyhammer-tools/src/mcp/tool_registry.rs`
- `swissarmyhammer-tools/src/mcp/responses.rs`
- `swissarmyhammer-tools/src/mcp/types.rs`
- `swissarmyhammer-tools/src/mcp/tools/issues/` (all issue tools)

## Implementation Steps

1. Create new `swissarmyhammer-issues` crate in workspace
2. Move issue-related code from main crate to new crate
3. Update `swissarmyhammer-tools` to depend on `swissarmyhammer-issues`
4. Update all imports from `swissarmyhammer::issues::` to `swissarmyhammer_issues::`
5. Remove issues module from main crate
6. Ensure issue-git integration works independently

## Acceptance Criteria

- [ ] New `swissarmyhammer-issues` crate created
- [ ] All issue functionality moved and working independently
- [ ] `swissarmyhammer-tools` uses new crate directly
- [ ] Issue creation, completion, merging works
- [ ] Git branch operations for issues work
- [ ] All issue tests pass
- [ ] No dependency on main `swissarmyhammer` crate