# Research Current Tool Implementations

Refer to ./specification/issue_current.md

## Goal

Understand the complete implementation of `issue_current` and `issue_next` tools before consolidating them into `issue_show`.

## Tasks

1. **Analyze issue_current tool implementation**:
   - Read `/swissarmyhammer/src/mcp/tools/issues/current/mod.rs`
   - Understand git branch parsing logic
   - Identify current branch prefix handling logic
   - Document key functionality that needs to be preserved

2. **Analyze issue_next tool implementation**:
   - Read `/swissarmyhammer/src/mcp/tools/issues/next/mod.rs`
   - Understand next issue selection logic
   - Identify how it uses storage backend to find pending issues
   - Document key functionality that needs to be preserved

3. **Understand current issue_show tool**:
   - Read `/swissarmyhammer/src/mcp/tools/issues/show/mod.rs`
   - Understand current parameter handling
   - Identify extension points for new functionality
   - Document current behavior to ensure backward compatibility

4. **Document findings**:
   - Create detailed analysis of what needs to be integrated
   - Identify potential conflicts or edge cases
   - Plan the integration approach

## Expected Outcome

Clear understanding of:
- How current branch parsing works in `issue_current`
- How next issue selection works in `issue_next`
- Current `issue_show` implementation structure
- Integration strategy for consolidating functionality

## Success Criteria

- All three tool implementations are fully understood
- Integration approach is documented and planned
- No functionality will be lost in the consolidation
- Backward compatibility is preserved for existing `issue_show` usage

## Proposed Solution

After analyzing all three tool implementations, I have a clear understanding of how to consolidate the functionality:

### Key Findings

**issue_current tool (`swissarmyhammer/src/mcp/tools/issues/current/mod.rs`)**:
- Uses git operations to get current branch
- Strips `Config::global().issue_branch_prefix` (default: "issue/") to extract issue name
- Returns success message: "Currently working on issue: {issue_name}" when on issue branch
- Returns success message: "Not on an issue branch. Current branch: {branch}" when not on issue branch
- Has optional `branch` parameter (unused in current implementation)
- Uses `CurrentIssueRequest` struct with single optional field

**issue_next tool (`swissarmyhammer/src/mcp/tools/issues/next/mod.rs`)**:
- Uses `issue_storage.get_next_issue().await` method
- The storage method returns first non-completed issue from `list_issues()`
- Returns success message: "Next issue: {issue_name}" when issue found
- Returns success message: "No pending issues found. All issues are completed!" when none found
- Takes no parameters (uses empty `NextIssueRequest` struct)

**issue_show tool (`swissarmyhammer/src/mcp/tools/issues/show/mod.rs`)**:
- Current implementation looks up issues by exact name match
- Has structured `ShowIssueRequest` with `name: String` and optional `raw: bool`
- Uses `Self::format_issue_display()` for rich formatting
- Supports raw content output when `raw: true`
- Has rate limiting, validation, and comprehensive error handling
- Current schema requires `name` parameter

### Integration Strategy

**1. Enhance ShowIssueTool.execute() method**:
- Detect special `name` values: `"current"` and `"next"`
- For `"current"`: replicate issue_current logic using git_ops
- For `"next"`: use storage.get_next_issue() like issue_next tool
- For regular names: preserve existing lookup behavior

**2. Key implementation details**:
- Add git operations access to ShowIssueTool via ToolContext
- Integrate `Config::global().issue_branch_prefix` handling
- Use existing `format_issue_display()` for consistent formatting
- Maintain backward compatibility for regular issue names
- Preserve `raw` parameter functionality for all modes

**3. Response format changes**:
- **Current tool**: Returns just status message, needs to show full issue details
- **Next tool**: Returns just name, needs to show full issue details  
- **Enhanced show**: Will return complete formatted issue details for all cases

**4. Error handling**:
- Current tool: "Not on issue branch" → should return error when `name="current"` and not on issue branch
- Next tool: "No pending issues" → should return appropriate message when `name="next"` and no issues
- Show tool: Preserve existing validation and error patterns

### Implementation Plan

1. **Modify ShowIssueTool schema**: Make `name` parameter accept special values
2. **Add branch detection logic**: Integrate git_ops functionality
3. **Add next issue logic**: Use storage.get_next_issue() method
4. **Update response handling**: Return full issue details instead of just names/messages
5. **Preserve existing behavior**: Ensure regular issue names work unchanged
6. **Update tool description**: Document new "current" and "next" special values

This approach maintains all existing functionality while consolidating the tools into a single, more powerful interface.