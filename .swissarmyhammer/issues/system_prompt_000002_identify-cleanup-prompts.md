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

## Proposed Solution

Based on my systematic analysis, I have identified and categorized all prompts containing template includes for removal. The analysis reveals 16 builtin prompt files need modification, with clear patterns for systematic removal.

### Analysis Results

#### Validated File List (16 files confirmed)
**Root prompts (3 files):**
- `builtin/prompts/test.md` - principals, coding_standards
- `builtin/prompts/coverage.md` - principals, coding_standards  
- `builtin/prompts/plan.md` - principals, coding_standards

**Review prompts (5 files):**
- `builtin/prompts/review/security.md` - principals, coding_standards
- `builtin/prompts/review/code.md` - principals, coding_standards
- `builtin/prompts/review/patterns.md` - principals, coding_standards
- `builtin/prompts/review/placeholders.md` - principals, coding_standards
- `builtin/prompts/review/accessibility.md` - principals, coding_standards

**Issue prompts (3 files):**
- `builtin/prompts/issue/review.md` - principals, coding_standards
- `builtin/prompts/issue/code.md` - principals, coding_standards
- `builtin/prompts/issue/code_review.md` - principals, coding_standards

**Documentation prompts (5 files):**
- `builtin/prompts/docs/review.md` - principals only
- `builtin/prompts/docs/readme.md` - principals only
- `builtin/prompts/docs/correct.md` - principals only
- `builtin/prompts/docs/project.md` - principals only
- `builtin/prompts/docs/comments.md` - principals only

#### Template Usage Patterns

**Template includes found:**
- `{% render "principals" %}` - 16 files (all affected files)
- `{% render "coding_standards" %}` - 11 files (root, review, and issue prompts)
- `{% render "tool_use" %}` - 0 files (not used in builtin prompts, only in .system.md)

**Template source files identified:**
- `/builtin/prompts/principals.md.liquid` - Contains motivational AI messaging and work principles
- `/builtin/prompts/coding_standards.md.liquid` - Contains comprehensive coding standards and language-specific rules
- `/builtin/prompts/tool_use.md.liquid` - Contains search and memo usage guidelines

#### Impact Analysis

**Low Impact Files (5 files) - Safe for immediate removal:**
Documentation prompts that only include `principals` template. These are task-specific prompts where the motivational content is not critical to functionality.

**Medium Impact Files (8 files) - Requires careful removal:**
Review and issue prompts that include both `principals` and `coding_standards`. The `coding_standards` content is more functionally important for code quality, but these prompts have other specific guidance.

**Higher Impact Files (3 files) - Requires validation:**
Root prompts (test, coverage, plan) that are core workflow prompts. These may rely more heavily on the included standards for proper operation.

### Removal Strategy

**Phase 1: Documentation Prompts (Safest)**
Order: `docs/correct.md`, `docs/comments.md`, `docs/readme.md`, `docs/review.md`, `docs/project.md`
- Only contain `principals` include
- Task-specific guidance is primary function
- Low risk of breaking functionality

**Phase 2: Review Prompts** 
Order: `review/placeholders.md`, `review/accessibility.md`, `review/patterns.md`, `review/security.md`, `review/code.md`
- Contain both includes but have domain-specific focus
- Built-in review logic is primary function
- Medium risk, but good domain separation

**Phase 3: Issue Prompts**
Order: `issue/review.md`, `issue/code_review.md`, `issue/code.md`
- Critical workflow prompts that guide implementation
- Most likely to need the standards for proper function
- Should be tested carefully after removal

**Phase 4: Root Prompts (Most Critical)**
Order: `coverage.md`, `plan.md`, `test.md`
- Core system prompts that drive development workflow
- Highest likelihood of depending on included content
- Should be done last with thorough testing

### Implementation Steps

1. **Create test cases** for each prompt category to verify functionality before and after removal
2. **Remove includes in phases** starting with documentation prompts
3. **Validate behavior** after each phase using existing prompts
4. **Document any functional changes** needed in prompts after include removal
5. **Verify .system.md integration** works as expected with `--append-system-prompt`

### Risk Mitigation

- Process files in low-to-high risk order
- Test prompt functionality after each phase
- Keep template content available for reference during testing
- Validate that .system.md contains equivalent content for system prompt injection

The systematic approach ensures we maintain prompt functionality while successfully removing template includes as specified in the system prompt infrastructure change.