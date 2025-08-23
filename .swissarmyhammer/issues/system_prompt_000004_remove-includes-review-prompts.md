# Remove Template Includes from Review Category Prompts

Refer to /Users/wballard/github/swissarmyhammer/ideas/system_prompt.md

## Overview
Remove template includes from the review category builtin prompts. These prompts focus on code and documentation review tasks.

## Target Files (6 files)
1. `builtin/prompts/review/security.md` - Contains principals + coding_standards
2. `builtin/prompts/review/code.md` - Contains principals + coding_standards
3. `builtin/prompts/review/patterns.md` - Contains principals + coding_standards
4. `builtin/prompts/review/placeholders.md` - Contains principals + coding_standards
5. `builtin/prompts/review/accessibility.md` - Contains principals + coding_standards

## Implementation Steps

### For each review prompt:

1. **Analyze prompt context**
   - Review prompts are typically used for quality assurance
   - Template includes provide coding standards for review criteria
   - Understand how standards integrate with review instructions

2. **Remove template include lines**
   - Remove `{% render "principals" %}` lines
   - Remove `{% render "coding_standards" %}` lines
   - Preserve all other prompt content and logic

3. **Verify review functionality**
   - Ensure review prompts still provide clear guidance
   - Confirm review criteria remain comprehensive
   - Test that prompts work without explicit standards injection

4. **Validate rendering and functionality**
   - Test each prompt with `sah prompt render <name>`
   - Verify prompts maintain their review focus
   - Ensure no broken template references

## Success Criteria
- ✅ Template includes removed from all 6 review prompts
- ✅ Review functionality and guidance preserved
- ✅ Prompts render correctly without includes
- ✅ No syntax errors or broken references
- ✅ Review quality and comprehensiveness maintained

## Special Considerations

### Review Context Importance
- Review prompts rely heavily on coding standards for evaluation criteria
- Standards will now come from system prompt instead of explicit includes
- Need to verify review prompts remain effective without explicit standards

### Testing Approach
- Test prompts with actual code review scenarios
- Verify review guidance is still comprehensive
- Ensure standards are still effectively applied through system prompt

## Technical Notes
- Review prompts are critical for code quality workflows
- Standards integration is essential for effective reviews
- System prompt should provide same standards context
- May need to adjust prompt language if standards context changes

## Risk Assessment
- **Medium risk**: Review prompts depend heavily on coding standards
- **Mitigation**: Thorough testing with actual review scenarios
- **Monitoring**: Watch for any degradation in review quality