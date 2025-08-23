# Rename standards.md to .system.md

Refer to /Users/wballard/github/swissarmyhammer/ideas/system_prompt.md

## Overview
Rename the built-in `standards.md` prompt to `.system.md` as the foundation for the new system prompt infrastructure.

## Current State
- `builtin/prompts/standards.md` contains template renders for principals, coding_standards, and tool_use
- This file serves as the comprehensive standards injection point
- File is currently used via explicit prompt calls

## Implementation Steps

1. **Rename the file**
   - Move `builtin/prompts/standards.md` to `builtin/prompts/.system.md`
   - Preserve all existing content and structure

2. **Update frontmatter if needed**
   - Review frontmatter properties for system prompt usage
   - Ensure name/title reflect system prompt purpose

3. **Validate rendering**
   - Test that the renamed file renders correctly
   - Verify all template includes (principals, coding_standards, tool_use) still work
   - Confirm no broken references

## Success Criteria
- ✅ File successfully renamed from `standards.md` to `.system.md`
- ✅ All template includes render correctly in the new location
- ✅ No build errors or broken references
- ✅ File maintains same functionality as before rename

## Technical Notes
- The dot prefix (`.system.md`) follows convention for system-level files
- Content should remain identical for this step
- This is purely a structural change to prepare for system prompt integration

## Testing
- Use `sah prompt render .system` to verify rendering works
- Confirm all three template includes (principals, coding_standards, tool_use) are present in output