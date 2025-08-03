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