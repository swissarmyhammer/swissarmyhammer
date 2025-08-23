# Remove Template Includes from Root Builtin Prompts

Refer to /Users/wballard/github/swissarmyhammer/ideas/system_prompt.md

## Overview
Remove template includes (`{% render "principals" %}`, `{% render "coding_standards" %}`, `{% render "tool_use" %}`) from the root-level builtin prompts.

## Target Files (3 files)
1. `builtin/prompts/test.md` - Contains principals + coding_standards
2. `builtin/prompts/coverage.md` - Contains principals + coding_standards  
3. `builtin/prompts/plan.md` - Contains principals + coding_standards

## Implementation Steps

### For each target file:

1. **Read and analyze current content**
   - Examine the prompt structure and context
   - Identify exact location of template includes
   - Understand how includes fit into the prompt logic

2. **Remove template include lines**
   - Remove lines containing `{% render "principals" %}`
   - Remove lines containing `{% render "coding_standards" %}`
   - Remove lines containing `{% render "tool_use" %}` (if present)

3. **Clean up formatting**
   - Remove any extra blank lines left by removal
   - Ensure proper spacing and flow
   - Maintain prompt readability

4. **Validate prompt still works**
   - Test prompt rendering with `sah prompt render <name>`
   - Verify prompt functionality is preserved
   - Ensure no broken references or syntax errors

## Success Criteria
- ✅ All template includes removed from 3 root prompt files
- ✅ Prompts render correctly without includes
- ✅ No syntax errors or broken references
- ✅ Prompt functionality preserved
- ✅ Clean formatting maintained

## Testing Strategy
- Render each prompt before and after changes
- Compare output to ensure only template includes are removed
- Verify prompts can be used in actual workflows
- Test with `sah prompt test <name>` if available

## Technical Notes
- These are the simplest prompts to start with (root level, clear includes)
- Template content will now come from system prompt instead
- No functional changes to prompt behavior expected
- This establishes the pattern for remaining prompt cleanups

## Risk Assessment
- **Low risk**: Root prompts have straightforward template include usage
- **Mitigation**: Test each prompt after modification
- **Rollback**: Can easily restore from git if needed