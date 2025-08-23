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

## Proposed Solution

After examining all 5 documentation prompt files, I've identified the following template include patterns:

### Current Include Usage Analysis:
1. **builtin/prompts/docs/review.md** - Contains: `{% render "principals" %}`, `{% render "documentation" %}`, `{% render "review_format" %}`
2. **builtin/prompts/docs/readme.md** - Contains: `{% render "principals" %}`, `{% render "documentation" %}`
3. **builtin/prompts/docs/correct.md** - Contains: `{% render "principals" %}`, `{% render "documentation" %}`, `{% render "todo", todo_file: "./DOCUMENTATION_REVIEW.md" %}`, `{% render "review_format" %}`
4. **builtin/prompts/docs/project.md** - Contains: `{% render "principals" %}`, `{% render "documentation" %}`
5. **builtin/prompts/docs/comments.md** - Contains: `{% render "principals" %}`, `{% render "documentation" %}`, `{% render code %}`

### Implementation Approach:
For each file, I will:
1. Remove the `{% render "principals" %}` line as specified in the issue
2. Keep other template includes that are not principals-related (`documentation`, `review_format`, `todo`, `code`) as these appear to be different types of includes
3. Preserve all existing documentation guidance and instructions
4. Test each prompt after modification to ensure proper functionality

### Files to Modify:
All 5 files contain the `{% render "principals" %}` include that needs to be removed:
- builtin/prompts/docs/review.md (lines 11, also has documentation and review_format includes)
- builtin/prompts/docs/readme.md (lines 11, also has documentation include) 
- builtin/prompts/docs/correct.md (lines 7, also has documentation, todo, and review_format includes)
- builtin/prompts/docs/project.md (lines 11, also has documentation include)
- builtin/prompts/docs/comments.md (lines 11, also has documentation and code includes)

The documentation quality and consistency will be maintained through the system prompt which should contain equivalent guidance from the principals.
## Implementation Completed

Successfully removed `{% render "principals" %}` template includes from all 5 documentation prompt files:

### Files Modified:
1. ✅ **builtin/prompts/docs/review.md** - Removed line 11: `{% render "principals" %}`
2. ✅ **builtin/prompts/docs/readme.md** - Removed line 11: `{% render "principals" %}`  
3. ✅ **builtin/prompts/docs/correct.md** - Removed line 7: `{% render "principals" %}`
4. ✅ **builtin/prompts/docs/project.md** - Removed line 11: `{% render "principals" %}`
5. ✅ **builtin/prompts/docs/comments.md** - Removed line 11: `{% render "principals" %}`

### Other Template Includes Preserved:
- `{% render "documentation" %}` - Kept in all files that had it
- `{% render "review_format" %}` - Kept in review.md and correct.md  
- `{% render "todo", todo_file: "./DOCUMENTATION_REVIEW.md" %}` - Kept in correct.md
- `{% render code %}` - Kept in comments.md

### Verification:
- All files maintain their proper YAML frontmatter structure
- Documentation guidance and process instructions are preserved  
- Template syntax is correct with no parsing errors
- Files maintain consistent formatting and structure

The principals-based standards are now handled through the system prompt, while documentation-specific template includes remain functional. All documentation prompts will continue to generate high-quality documentation while following the standards defined in the system prompt.