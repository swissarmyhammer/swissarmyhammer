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
## Analysis Completed

### Current State Analysis

I have read and analyzed all three target files:

1. **`builtin/prompts/test.md`** - Lines 12-13 contain:
   - `{% render "principals" %}`
   - `{% render "coding_standards" %}`

2. **`builtin/prompts/coverage.md`** - Lines 11-12 contain:
   - `{% render "principals" %}`
   - `{% render "coding_standards" %}`

3. **`builtin/prompts/plan.md`** - Lines 22-23 contain:
   - `{% render "principals" %}`
   - `{% render "coding_standards" %}`

### Template Include Locations

All three files follow the exact same pattern:
- Template includes appear after the main goal/description section
- Both principals and coding_standards are rendered in sequence
- No tool_use template includes found in these root prompts
- Clean formatting with proper spacing around the includes

## Proposed Solution

I will remove the template include lines from each file:

1. **For each file**: Remove lines containing `{% render "principals" %}` and `{% render "coding_standards" %}`
2. **Clean formatting**: Ensure proper spacing is maintained after removal
3. **Test rendering**: Verify each prompt still renders correctly without syntax errors

This is a straightforward text removal operation that should preserve all other prompt functionality while removing the template dependencies.
## Implementation Completed ✅

### Summary

Successfully removed template includes from all three root builtin prompts:

1. **`builtin/prompts/test.md`** ✅
   - Removed `{% render "principals" %}` from line 12
   - Removed `{% render "coding_standards" %}` from line 13
   - Prompt renders correctly with full content

2. **`builtin/prompts/coverage.md`** ✅  
   - Removed `{% render "principals" %}` from line 11
   - Removed `{% render "coding_standards" %}` from line 12
   - Prompt renders correctly with full content

3. **`builtin/prompts/plan.md`** ✅
   - Removed `{% render "principals" %}` from line 22
   - Removed `{% render "coding_standards" %}` from line 23
   - Prompt renders correctly with full content

### Validation Results

All three prompts tested successfully with `sah prompt test <name>`:
- **test**: Renders with complete principals and coding standards content
- **coverage**: Renders with complete principals and coding standards content  
- **plan**: Renders with complete principals and coding standards content

The content that was previously included via templates is now coming directly from the system prompt, ensuring consistent behavior while removing the template dependencies.

### Technical Notes

- Clean removal with proper formatting preserved
- No syntax errors or broken references 
- Full functionality maintained
- Template content now sourced from system prompt
- Establishes pattern for remaining prompt cleanup tasks