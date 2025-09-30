# Step 7: Create Tool Description Documentation

Refer to ideas/changes.md

## Objective

Create comprehensive `description.md` for the git_changes tool.

## Tasks

1. Create `swissarmyhammer-tools/src/mcp/tools/git/changes/description.md`
   - Tool title and purpose
   - Parameters section with examples
   - Returns section with response format
   - Edge cases documentation
   - Usage examples (feature branch, main branch, uncommitted changes)
   - Error conditions

2. Follow existing description.md pattern from other tools
   - Use markdown formatting
   - Include JSON examples
   - Document all parameters
   - Provide multiple usage scenarios

3. Include key concepts:
   - Parent branch detection
   - Merge-base calculation
   - Root branch vs feature branch behavior
   - Uncommitted changes inclusion

## Success Criteria

- description.md exists and is comprehensive
- Follows established pattern from other tools
- All parameters and responses documented
- Examples are clear and useful
- Edge cases are explained

## Files to Create

- `swissarmyhammer-tools/src/mcp/tools/git/changes/description.md`

## Estimated Code Changes

~100 lines (documentation)