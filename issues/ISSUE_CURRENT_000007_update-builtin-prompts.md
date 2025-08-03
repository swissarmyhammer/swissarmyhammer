# Update Builtin Prompts to Use Enhanced issue_show

Refer to ./specification/issue_current.md

## Goal

Update all builtin prompt files to use the new `issue_show current` and `issue_show next` syntax instead of the deprecated `issue_current` and `issue_next` tools.

## Tasks

1. **Update issue/code.md prompt**:
   - Change references from `issue_current` to `issue_show current`
   - Verify context and usage remain appropriate
   - Test prompt functionality after changes

2. **Update issue/complete.md prompt**:
   - Change references from `issue_current` to `issue_show current`
   - Verify context and usage remain appropriate
   - Test prompt functionality after changes

3. **Update issue/on_worktree.md.liquid prompt**:
   - Change references from `issue_current` to `issue_show current`
   - Change references from `issue_next` to `issue_show next`
   - Handle liquid template processing correctly
   - Test prompt functionality after changes

4. **Search for additional references**:
   - Search for any other builtin prompts that might reference these tools
   - Update any found references consistently
   - Document any edge cases or special handling

5. **Validate prompt functionality**:
   - Test each updated prompt works correctly
   - Ensure prompt logic and flow remain unchanged
   - Verify output and behavior match expectations
   - Test with various issue states and scenarios

6. **Update any related documentation**:
   - Update any documentation that references these tools
   - Ensure examples and usage guides are current
   - Maintain consistency across all documentation

## Expected Outcome

All builtin prompts work identically to before but use the new consolidated tool syntax:
- Same functionality and behavior
- Same user experience
- Same prompt logic and flow
- Updated tool references throughout

## Success Criteria

- All identified prompt files are updated correctly
- No references to old tools remain in builtin prompts
- All prompts continue to work exactly as before
- Prompt testing validates correct functionality
- Documentation is consistent and up-to-date