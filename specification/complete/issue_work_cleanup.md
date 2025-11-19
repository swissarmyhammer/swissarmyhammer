# OBSOLETE: Issue Work Cleanup Plan

**Status**: This specification is obsolete. The entire issue system has been removed.

**Replacement**: Work directly with git commands and the todos system. No specialized branch management tools are provided.

---

# Original Specification

_The content below describes a cleanup plan that was superseded by complete removal of the issue system._

## Overview

Remove the `issue_work` and `issue_merge` tools and eliminate the git branching workflow from the issue management system. Issues will be worked on the current branch without creating separate `issue/<name>` branches.

## Motivation

- Simplify the issue workflow by eliminating branch management complexity
- Work directly on the current branch (typically `main` or feature branches)
- Remove the need to merge issue branches back to source branches
- Still track "current issue" context but without branching

## Impact Analysis

### Tools to Remove

1. **issue_work** (`swissarmyhammer-tools/src/mcp/tools/issues/work/mod.rs`)
   - Currently switches to `issue/<issue_name>` branch
   - Creates branch if it doesn't exist
   - Used by prompts and workflows

2. **issue_merge** (`swissarmyhammer-tools/src/mcp/tools/issues/merge/mod.rs`)
   - Currently merges issue branch back to source branch
   - Optionally deletes the branch after merge
   - Used in completion workflows

### Current Issue Tracking

**Keep but modify**: The concept of "current issue" should remain, but determined by:
- A marker file (e.g., `.swissarmyhammer/.current_issue`) containing the issue name
- No longer determined by branch name pattern matching
- `issue_show current` should read from this marker file instead of git branch

### Files to Modify

#### Source Code

1. **Tool Implementations**
   - Delete: `swissarmyhammer-tools/src/mcp/tools/issues/work/mod.rs`
   - Delete: `swissarmyhammer-tools/src/mcp/tools/issues/merge/mod.rs`
   - Update: Tool registry to remove these tools
   - Update: `issue_show` to use marker file instead of git branch for "current"

2. **Tests to Remove**
   - `swissarmyhammer-cli/tests/flexible_branching_mcp_e2e.rs`
     - `test_mcp_issue_work_from_feature_branch()` (line 136)
     - `test_mcp_issue_work_from_develop_branch()` (line 189)
     - `test_mcp_issue_work_prevents_issue_from_issue_branch()` (line 347)
     - `test_mcp_issue_merge_requires_issue_branch()` (line 216)
     - `test_mcp_issue_merge_to_source_branch()` (line 280)
   - `swissarmyhammer-cli/tests/cli_mcp_integration_test.rs`
     - `test_issue_workflow_integration()` (line 205) - update or remove
   - `swissarmyhammer/tests/flexible_branching_integration.rs`
     - `test_release_branch_issue_workflow()` (line 375) - update or remove
   - `swissarmyhammer/tests/mcp_issue_integration_tests.rs`
     - `test_complete_issue_workflow()` (line 90) - update or remove

3. **Test Assertions to Update**
   - `swissarmyhammer-tools/tests/mcp_server_parity_tests.rs:83` - Remove "issue_work" from list
   - `swissarmyhammer-tools/tests/mcp_server_parity_tests.rs:84` - Remove "issue_merge" from list
   - `swissarmyhammer-cli/tests/sah_serve_integration_test.rs:64` - Remove "issue_work" from list

#### Documentation

1. **Prompt Files**
   - `builtin/prompts/issue/code.md:25` - Remove `issue_work` tool usage, add logic to set current issue marker
   - `builtin/prompts/issue/merge.md` - Delete entire file (workflow no longer relevant)

2. **Tool Documentation**
   - Delete: `doc/src/05-tools/issue-management/work.md`
   - Delete: `doc/src/05-tools/issue-management/merge.md`
   - Update: `doc/src/05-tools/issue-management/introduction.md:28` - Remove `issue_work` from list
   - Update: `doc/src/05-tools/issue-management/introduction.md:30` - Remove `issue_merge` from list
   - Update: `doc/src/SUMMARY.md:43` - Remove `issue_work` link
   - Update: `doc/src/SUMMARY.md:45` - Remove `issue_merge` link

3. **General Documentation**
   - `doc/src/02-concepts/architecture.md:214` - Remove `issue_work` from tool list
   - `doc/src/05-tools/overview.md:18` - Remove `issue_work` entry
   - `doc/src/06-integration/cli-usage.md:105` - Remove `issue_work` row from table
   - `doc/src/06-integration/claude-code.md:35` - Remove `issue_work` from list
   - `doc/src/06-integration/claude-code.md:134` - Remove example
   - `doc/src/06-integration/claude-code.md:220` - Remove from workflow example
   - `doc/src/06-integration/claude-code.md:293` - Remove from step list
   - `swissarmyhammer-cli/src/commands/serve/description.md:125` - Remove mention of branch management tools

4. **Workflow Documentation**
   - `doc/src/04-workflows/custom-workflows.md:142` - Remove `issue_work` step
   - `doc/src/04-workflows/custom-workflows.md:528` - Remove `issue_work` step
   - `doc/src/04-workflows/custom-workflows.md:671` - Remove `issue_work` step
   - `doc/src/04-workflows/custom-workflows.md:842` - Remove `issue_work` step
   - `doc/src/04-workflows/custom-workflows.md:754` - Remove `issue_merge` step
   - `doc/src/04-workflows/custom-workflows.md:904` - Remove `issue_merge` step

## Implementation Steps

### Phase 1: Add Current Issue Marker System

1. Create `.swissarmyhammer/.current_issue` marker file mechanism
2. Update `issue_show current` to read from marker file instead of git branch
3. Add helper functions to set/get/clear current issue marker
4. Update prompts to set the marker when starting work on an issue

### Phase 2: Remove Tools

1. Delete `swissarmyhammer-tools/src/mcp/tools/issues/work/mod.rs`
2. Delete `swissarmyhammer-tools/src/mcp/tools/issues/merge/mod.rs`
3. Update tool registry to remove these tools
4. Update any internal code that references these tools

### Phase 3: Update Tests

1. Remove branching-specific tests
2. Update integration tests to not expect branching behavior
3. Update test assertions that check for tool existence
4. Ensure remaining issue management tests still pass

### Phase 4: Update Documentation

1. Delete tool documentation pages for `issue_work` and `issue_merge`
2. Update all documentation references to remove these tools
3. Update workflow examples to show new non-branching workflow
4. Update architecture documentation
5. Update prompt files to use new marker-based current issue tracking

### Phase 5: Update Prompts

1. Modify `builtin/prompts/issue/code.md` to set marker instead of using `issue_work`
2. Delete `builtin/prompts/issue/merge.md` prompt
3. Update any other prompts that reference these tools
4. Update workflow YAML files that reference these tools

## Migration Notes

### For Users

- Issues will be worked on whatever branch you're currently on
- No more automatic branch creation or switching
- You can still use git branches manually if desired
- Issue tracking and completion still works the same way
- "Current issue" is now tracked in `.swissarmyhammer/.current_issue`

### For Workflows

- Remove `issue_work` steps from custom workflows
- Remove `issue_merge` steps from custom workflows
- Workflows will work on current branch
- Git operations (commit, push) are still user-controlled

## Files Summary

### To Delete
- `swissarmyhammer-tools/src/mcp/tools/issues/work/mod.rs`
- `swissarmyhammer-tools/src/mcp/tools/issues/merge/mod.rs`
- `builtin/prompts/issue/merge.md`
- `doc/src/05-tools/issue-management/work.md`
- `doc/src/05-tools/issue-management/merge.md`

### To Modify
- Tool registry (remove tool registrations)
- `issue_show` tool (update "current" logic)
- Test files (remove branching tests, update assertions)
- Documentation files (17 files with references)
- Prompt files (1-2 files)
- Workflow YAML files (multiple references)

### To Create
- Current issue marker file mechanism
- Helper functions for marker management
- Updated documentation explaining new workflow

## Testing Strategy

1. Test that `issue_show current` works with marker file
2. Test that issue workflow works without branching
3. Test that all remaining issue tools work correctly
4. Verify documentation builds without broken links
5. Test example workflows without branching steps
