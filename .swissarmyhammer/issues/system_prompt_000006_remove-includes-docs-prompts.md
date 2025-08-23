# Remove Template Includes from Documentation Category Prompts

Refer to /Users/wballard/github/swissarmyhammer/ideas/system_prompt.md

## Overview
Remove template includes from documentation-focused builtin prompts. These prompts are used for generating and maintaining project documentation.

## Target Files (5 files)
1. `builtin/prompts/docs/review.md` - Contains principals
2. `builtin/prompts/docs/readme.md` - Contains principals
3. `builtin/prompts/docs/correct.md` - Contains principals
4. `builtin/prompts/docs/project.md` - Contains principals
5. `builtin/prompts/docs/comments.md` - Contains principals

## Implementation Steps

### For each documentation prompt:

1. **Analyze documentation context**
   - Documentation prompts focus on creating and maintaining docs
   - Template includes provide standards for documentation quality
   - Understand how standards guide documentation structure and style

2. **Remove template include lines**
   - Remove `{% render "principals" %}` lines
   - Note: Most docs prompts only include principals, not coding_standards
   - Preserve all documentation-specific guidance and instructions

3. **Maintain documentation quality standards**
   - Ensure prompts still provide clear documentation guidance
   - Verify documentation structure and style requirements remain clear
   - Confirm quality standards are maintained through system prompt

4. **Test documentation generation**
   - Validate prompts produce high-quality documentation
   - Test with actual documentation scenarios
   - Verify consistency across different documentation types

## Success Criteria
- ✅ Template includes removed from all 5 documentation prompts
- ✅ Documentation quality and guidance preserved
- ✅ Documentation structure and style standards maintained
- ✅ Prompts render correctly without syntax errors
- ✅ Generated documentation meets quality standards

## Documentation Standards Considerations

### Quality and Consistency
- Documentation prompts ensure consistent quality across projects
- Principals guide documentation structure and approach
- System prompt must maintain same quality standards

### Documentation Types
- Different prompt types: README, project docs, code comments, reviews
- Each type has specific requirements and standards
- Standards must apply consistently across all documentation types

## Testing Strategy
- Generate documentation samples with each prompt
- Compare output quality before and after changes
- Test with various project types and scenarios
- Verify documentation meets established standards

## Technical Notes
- Documentation prompts primarily use principals (not coding_standards)
- Standards ensure consistent documentation approach
- System prompt should provide equivalent guidance
- Focus on maintaining documentation quality and consistency

## Risk Assessment
- **Low-Medium risk**: Documentation prompts have simpler include patterns
- **Quality impact**: Must maintain documentation standards
- **Mitigation**: Test with actual documentation generation scenarios