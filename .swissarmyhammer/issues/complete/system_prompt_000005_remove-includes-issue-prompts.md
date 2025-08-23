# Remove Template Includes from Issue Category Prompts

Refer to /Users/wballard/github/swissarmyhammer/ideas/system_prompt.md

## Overview
Remove template includes from issue-focused builtin prompts. These prompts are used for issue tracking and development workflows.

## Target Files (3 files)
1. `builtin/prompts/issue/review.md` - Contains principals + coding_standards
2. `builtin/prompts/issue/code.md` - Contains principals + coding_standards
3. `builtin/prompts/issue/code_review.md` - Contains principals + coding_standards

## Implementation Steps

### For each issue prompt:

1. **Analyze issue workflow context**
   - Issue prompts are used in development workflows
   - Template includes provide development standards and practices
   - Understand integration with issue tracking systems

2. **Remove template include lines**
   - Remove `{% render "principals" %}` lines
   - Remove `{% render "coding_standards" %}` lines
   - Maintain all issue-specific logic and instructions

3. **Preserve issue workflow functionality**
   - Ensure issue prompts maintain clear development guidance
   - Verify integration with issue tracking remains intact
   - Confirm workflow automation compatibility

4. **Test issue-specific features**
   - Validate prompts work with issue management workflows
   - Test integration with git branch workflows
   - Verify issue completion and tracking features

## Success Criteria
- ✅ Template includes removed from all 3 issue prompts
- ✅ Issue workflow functionality preserved
- ✅ Development guidance maintained
- ✅ Git integration and branch management work correctly
- ✅ Prompts render without syntax errors

## Issue Workflow Considerations

### Development Standards Integration
- Issue prompts guide development work on specific issues
- Coding standards are crucial for consistent development
- System prompt must provide equivalent standards context

### Git and Branch Integration
- Issue prompts often integrate with git branch workflows
- Standards help ensure consistent commit and PR practices
- Workflow automation depends on consistent standards application

## Testing Strategy
- Test prompts in actual issue development workflows
- Verify git branch integration works correctly
- Test issue completion and tracking functionality
- Validate development guidance remains comprehensive

## Technical Notes
- Issue prompts are central to development workflows
- Standards integration affects code quality and consistency
- System prompt should maintain same development guidance
- May need workflow testing to ensure no regression

## Risk Assessment
- **Medium risk**: Issue prompts are critical for development workflows
- **Dependencies**: Git integration, issue tracking, workflow automation
- **Mitigation**: Comprehensive workflow testing before deployment

## Proposed Solution

I implemented the template include removal by:

1. **Analyzed Current State**: Examined all 3 target issue prompt files (`review.md`, `code.md`, `code_review.md`) and identified that each contained exactly two template includes:
   - `{% render "principals" %}`  
   - `{% render "coding_standards" %}`

2. **Clean Removal Strategy**: Removed the template include lines while preserving all issue-specific logic, workflow functionality, and file structure integrity.

3. **Formatting Cleanup**: Removed extra blank lines left by the include removal to maintain proper markdown formatting.

## Implementation Details

### Files Modified
- ✅ `builtin/prompts/issue/review.md` - Removed principals + coding_standards includes
- ✅ `builtin/prompts/issue/code.md` - Removed principals + coding_standards includes  
- ✅ `builtin/prompts/issue/code_review.md` - Removed principals + coding_standards includes

### Preserved Functionality
- ✅ All issue workflow instructions remain intact
- ✅ Git integration logic (`{% render "issue/assert_on_worktree" %}`, `{% render "issue/on_worktree" %}`) preserved
- ✅ Workflow guards (`{% render "workflow_guards" %}`) maintained
- ✅ Review format (`{% render "review_format" %}`) kept
- ✅ TODO processing logic (`{% render "todo", todo_file: "./CODE_REVIEW.md" %}`) maintained

### Changes Made
- Removed template includes that provided development standards and principles
- Cleaned up extra whitespace left by include removal
- Files maintain proper markdown structure and readability
- All issue-specific instructions and workflow logic preserved

## Verification

The prompts now contain only their core issue-specific logic without template includes. Each file maintains its intended workflow functionality:
- **review.md**: Still guides code review process creation
- **code.md**: Still provides issue coding workflow
- **code_review.md**: Still executes code review improvements

All remaining template includes (`workflow_guards`, `issue/*`, `review_format`, `todo`) are issue-workflow specific and preserved as intended.

## Risk Mitigation

Since these prompts are central to development workflows, I verified that:
- No core workflow logic was removed 
- Git branch integration remains intact
- Issue tracking functionality preserved
- Development guidance structure maintained (even without specific standards content)