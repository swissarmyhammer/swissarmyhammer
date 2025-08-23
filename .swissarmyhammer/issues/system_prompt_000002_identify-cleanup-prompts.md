# Identify and Categorize Prompts for Template Include Removal

Refer to /Users/wballard/github/swissarmyhammer/ideas/system_prompt.md

## Overview
Systematically identify and categorize all prompts that contain template includes for `principals`, `coding_standards`, and `tool_use` that need to be removed.

## Current Analysis
Based on search results, 18 files contain these template includes:

### Builtin Prompts to Clean (17 files):
- `builtin/prompts/test.md`
- `builtin/prompts/coverage.md` 
- `builtin/prompts/plan.md`
- `builtin/prompts/review/security.md`
- `builtin/prompts/review/code.md`
- `builtin/prompts/review/patterns.md`
- `builtin/prompts/review/placeholders.md`
- `builtin/prompts/review/accessibility.md`
- `builtin/prompts/issue/review.md`
- `builtin/prompts/issue/code.md`
- `builtin/prompts/issue/code_review.md`
- `builtin/prompts/docs/review.md`
- `builtin/prompts/docs/readme.md`
- `builtin/prompts/docs/correct.md`
- `builtin/prompts/docs/project.md`
- `builtin/prompts/docs/comments.md`

### Specification Files (2 files - exclude):
- `specification/complete/plan.md` - Documentation, not operational
- `ideas/system_prompt.md` - The specification itself

## Implementation Steps

1. **Validate file list**
   - Verify each file exists and contains the expected template includes
   - Double-check search results for accuracy

2. **Categorize by prompt type**
   - Review prompts: 6 files (review/*.md)
   - Issue prompts: 3 files (issue/*.md)
   - Doc prompts: 5 files (docs/*.md)
   - Root prompts: 3 files (test.md, coverage.md, plan.md)

3. **Analyze impact of removal**
   - Review each prompt to understand how template includes are used
   - Identify any prompts that might break without these includes
   - Document any special cases or dependencies

4. **Create removal strategy**
   - Plan order of removal (start with safest/simplest)
   - Identify any prompts that may need content restructuring
   - Plan testing approach for each category

## Success Criteria
- ✅ Complete and accurate list of affected files verified
- ✅ Files categorized by type and impact level
- ✅ Removal strategy documented for next steps
- ✅ Impact analysis completed for each file

## Deliverables
- Validated list of 17 builtin prompt files to modify
- Categorization by prompt type for systematic removal
- Impact analysis identifying any special cases
- Recommended order for template include removal

## Notes
- Focus only on builtin/ prompts - specification files are documentation
- The .system.md file (formerly standards.md) should keep its includes
- This is preparation work - no actual file changes in this step