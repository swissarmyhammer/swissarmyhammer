# Analyze Builtin Prompt Usage

Refer to ./specification/issue_current.md

## Goal

Understand how `issue_current` and `issue_next` tools are currently used in builtin prompts to ensure seamless migration to `issue_show` with special parameters.

## Tasks

1. **Audit current usage**:
   - Search entire codebase for `issue_current` references
   - Search entire codebase for `issue_next` references
   - Document all usage locations and contexts

2. **Analyze specific builtin prompt files**:
   - `/builtin/prompts/issue/code.md` - references to `issue_current`
   - `/builtin/prompts/issue/complete.md` - references to `issue_current` 
   - `/builtin/prompts/issue/on_worktree.md.liquid` - references to both tools
   - Any other prompt files with references

3. **Document migration requirements**:
   - Map each `issue_current` call to `issue_show current`
   - Map each `issue_next` call to `issue_show next`  
   - Identify any contextual differences that need attention
   - Ensure no functionality changes in prompt behavior

4. **Validate complete coverage**:
   - Ensure all builtin prompt references are identified
   - Check for any template/liquid references that might be dynamic
   - Document test files that might need updating

## Expected Outcome

Complete mapping of:
- All current `issue_current` usage → `issue_show current`
- All current `issue_next` usage → `issue_show next`
- Test files that need updating
- Any edge cases or special handling required

## Success Criteria

- All builtin prompt usages are documented and migration planned
- No references will be missed in the actual migration
- Migration strategy preserves all current functionality
- Test files are identified for future updates
## Analysis Results

I have completed the comprehensive analysis of `issue_current` and `issue_next` tool usage across the entire codebase.

### Builtin Prompt Usage

**1. `/builtin/prompts/issue/on_worktree.md.liquid`**:
- Line 6: `use the issue_next tool to determine which issue to work`
- Line 8: `Use the issue_current tool -- this issue is what you are coding`

**2. `/builtin/prompts/issue/complete.md`**:
- Line 12: `use the issue_current tool to determine which issue is being worked`

**3. `/builtin/prompts/issue/code.md`**:
- References `on_worktree.md.liquid` via `{% render "issue/on_worktree" %}` (line 15), so inherits the tool references

### CLI Implementation Usage

**1. `/swissarmyhammer-cli/src/issue.rs`**:
- Line 183: `context.execute_tool("issue_current", args).await?` in `show_current_issue()` function
- Line 226: `context.execute_tool("issue_next", args).await?` in `show_next_issue()` function

### Test File Usage

**1. `/swissarmyhammer-cli/tests/comprehensive_cli_mcp_integration_tests.rs`**:
- Line 92: `context.execute_tool("issue_current", current_args).await` 
- Line 101: `context.execute_tool("issue_next", next_args).await`

**2. `/swissarmyhammer-cli/tests/cli_mcp_integration_test.rs`**:
- Line 171: `context.execute_tool("issue_next", next_args).await`

### Migration Requirements

**Builtin Prompts**:
1. `on_worktree.md.liquid:6`: `issue_next tool` → `issue_show next`
2. `on_worktree.md.liquid:8`: `issue_current tool` → `issue_show current`
3. `complete.md:12`: `issue_current tool` → `issue_show current`

**CLI Code**:
1. `issue.rs:183`: `"issue_current"` → `"issue_show"` with `name: "current"`
2. `issue.rs:226`: `"issue_next"` → `"issue_show"` with `name: "next"`

**Test Files**:
1. `comprehensive_cli_mcp_integration_tests.rs:92`: `"issue_current"` → `"issue_show"` with `name: "current"`
2. `comprehensive_cli_mcp_integration_tests.rs:101`: `"issue_next"` → `"issue_show"` with `name: "next"`
3. `cli_mcp_integration_test.rs:171`: `"issue_next"` → `"issue_show"` with `name: "next"`

### Coverage Summary

**Total References Found**:
- `issue_current`: 3 builtin prompt files, 1 CLI function, 1 test file = **5 active usage locations**
- `issue_next`: 2 builtin prompt files, 1 CLI function, 2 test files = **5 active usage locations**

**Complete Migration Map**:
- All builtin prompt text references need to change from "tool" to "tool with parameter"
- All CLI code calls need to switch from separate tools to single tool with parameters
- All test calls need similar parameter-based approach
- No dynamic/template references that would be missed

The analysis confirms that all usage is straightforward and the migration strategy is feasible with clear 1:1 mappings.

### Additional Reference Found

**4. `/doc/src/issue-management.md`**:
- Line 21: `[issue_current](#issue_current)` (table of contents)
- Line 171: `### issue_current` (section header)
- Line 181: `"tool": "issue_current"` (JSON example)

### Updated Migration Requirements

**Documentation Files**:
1. `doc/src/issue-management.md:21`: Update TOC reference 
2. `doc/src/issue-management.md:171`: Update section header
3. `doc/src/issue-management.md:181`: Update JSON example to show new parameter usage

### Final Coverage Summary

**Total References Found**:
- `issue_current`: 3 builtin prompt files, 1 CLI function, 1 test file, 1 documentation file = **6 active usage locations**
- `issue_next`: 2 builtin prompt files, 1 CLI function, 2 test files = **5 active usage locations**

All references have been identified and mapped for migration. No dynamic/template references found that would be missed during the migration process.