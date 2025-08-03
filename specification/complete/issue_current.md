# Remove issue_current and issue_next Tools

## Overview

This specification outlines the removal of the `issue_current` and `issue_next` MCP tools and their replacement with enhanced functionality in the existing `issue_show` tool.

## Current State

### Tools to be Removed

1. **issue_current**: Returns the current issue being worked on by parsing the current git branch
   - Implementation: `swissarmyhammer/src/mcp/tools/issues/current/mod.rs`
   - Returns: Information about the current issue based on branch name pattern

2. **issue_next**: Returns the first pending issue alphabetically by name
   - Implementation: `swissarmyhammer/src/mcp/tools/issues/next/mod.rs`
   - Returns: Next pending issue or message if all issues are complete

### Current Usage in Builtin Prompts

The following builtin prompt files currently use these tools and need to be updated:

1. **builtin/prompts/issue/code.md:16**
   - Current: `Use the issue_current tool -- this issue is what you are coding`
   - Needs replacement with: `Use the issue_show current tool -- this issue is what you are coding`

2. **builtin/prompts/issue/review.md:23**
   - Current: `use the issue_current tool to determine which issue to review`
   - Needs replacement with: `use the issue_show current tool to determine which issue to review`

3. **builtin/prompts/issue/complete.md:12**
   - Current: `use the issue_current tool to determine which issue is being worked`
   - Needs replacement with: `use the issue_show current tool to determine which issue is being worked`

4. **builtin/prompts/issue/on_worktree.md.liquid:6**
   - Current: `use the issue_next tool to determine which issue to work`
   - Needs replacement with: `use the issue_show current tool to determine which issue to work`

## Proposed Solution

### Enhance issue_show Tool

Modify the existing `issue_show` tool to accept special `name` parameter values:

#### `"current"` - Replace issue_current functionality
1. Get the current git branch
2. Parse the branch name to extract the issue name (using the same logic as `issue_current`)
3. Return the issue details for the current issue
4. If not on an issue branch, return appropriate error message

#### `"next"` - Replace issue_next functionality
1. Use the same logic as `issue_next` tool to find the first pending issue alphabetically
2. Return the issue details for the next pending issue
3. If no pending issues exist, return message indicating all issues are complete

### Implementation Changes Required

1. **Update ShowIssueTool in `swissarmyhammer/src/mcp/tools/issues/show/mod.rs`**:
   - Add logic to detect when `name` parameter equals `"current"` or `"next"`
   - Integrate current branch parsing logic from `issue_current` tool
   - Integrate next issue selection logic from `issue_next` tool
   - Handle case when not on an issue branch (for `"current"`)
   - Handle case when no pending issues exist (for `"next"`)

2. **Update tool description in `swissarmyhammer/src/mcp/tools/issues/show/description.md`**:
   - Document the special `"current"` and `"next"` name parameters
   - Add examples showing usage of `issue_show current` and `issue_show next`

3. **Remove tool implementations**:
   - Delete `swissarmyhammer/src/mcp/tools/issues/current/mod.rs`
   - Delete `swissarmyhammer/src/mcp/tools/issues/next/mod.rs`
   - Remove references from tool registry

4. **Update builtin prompts** (4 files identified):
   - Replace all `issue_current` calls with `issue_show current`
   - Replace `issue_next` call in `on_worktree.md.liquid` with `issue_show next`

### Updated Builtin Prompt Changes

With the addition of `issue_show next`, the builtin prompt in `on_worktree.md.liquid` can be updated to:
- Current: `use the issue_next tool to determine which issue to work`
- New: `use the issue_show next tool to determine which issue to work`

## Benefits

1. **Reduced API surface**: Two fewer MCP tools to maintain
2. **Consistent interface**: All issue querying goes through `issue_show`
3. **Simplified mental model**: One tool for showing issues with different behaviors based on parameter
4. **Maintainability**: Less code duplication and fewer tools to test

## Migration Path

1. Implement enhanced `issue_show` with `"current"` support
2. Update all builtin prompt files to use new syntax
3. Remove old tool implementations
4. Update documentation and tests
5. Verify all existing workflows still function correctly

## Testing Requirements

1. Verify `issue_show current` returns correct current issue when on issue branch
2. Verify appropriate error when not on issue branch
3. Verify `issue_show next` returns correct next pending issue alphabetically
4. Verify appropriate message when no pending issues exist for `issue_show next`
5. Test all updated builtin prompts work correctly
6. Ensure backward compatibility for normal `issue_show` usage with regular issue names
7. Integration tests for complete workflow using updated prompts

## Files Requiring Changes

### Core Implementation
- `swissarmyhammer/src/mcp/tools/issues/show/mod.rs`
- `swissarmyhammer/src/mcp/tools/issues/show/description.md`

### Tool Registry
- Remove registrations for `issue_current` and `issue_next` tools

### Builtin Prompts (4 files)
- `builtin/prompts/issue/code.md`
- `builtin/prompts/issue/review.md`
- `builtin/prompts/issue/complete.md`
- `builtin/prompts/issue/on_worktree.md.liquid`

### Files to Delete
- `swissarmyhammer/src/mcp/tools/issues/current/mod.rs`
- `swissarmyhammer/src/mcp/tools/issues/next/mod.rs`
- Any associated test files or type definitions

### Additional Files Found with References
- Various test files and completed issue files contain references but these are likely historical and don't need updating for functionality