# Update Tool Descriptions and Documentation for Flexible Branching

Refer to ./specification/flexible_base_branch_support.md

## Goal

Update all tool descriptions, help text, and documentation to reflect flexible base branch support instead of main branch requirements.

## Tasks

1. **Update MCP Tool Descriptions**  
   - Update issue work tool description to mention flexible base branch support
   - Update issue merge tool description to reference source branch instead of main
   - Remove references to "main branch" and replace with "base branch" or "source branch"
   - Update tool description files in MCP tool directories

2. **Update CLI Help Text**
   - Update command help text that mentions merging to main branch
   - Update error messages displayed to CLI users  
   - Update any hardcoded references to main branch in help text
   - Ensure consistency in terminology (use "source branch" throughout)

3. **Update Built-in Prompt Descriptions**
   - Review and update prompt descriptions that reference main branch workflows
   - Update workflow documentation that assumes main branch
   - Update any examples that hardcode main branch usage

4. **Update Code Documentation**
   - Update inline comments that reference main branch requirements
   - Update module documentation to reflect flexible branching
   - Update method documentation to describe source branch parameters

## Implementation Details

- Location: MCP tool description files, CLI help text, built-in prompts
- Search for "main branch", "merge to main", and similar phrases
- Replace with appropriate flexible branching terminology
- Ensure consistency across all user-facing text

## Testing Requirements

- Test that tool descriptions accurately reflect flexible branching behavior
- Test that help text provides correct information about branching workflows
- Test that error messages use consistent terminology
- Review all user-facing text for accuracy and consistency

## Success Criteria

- No references to hardcoded main branch requirements remain
- Tool descriptions accurately reflect flexible branching capabilities  
- Help text provides correct guidance for flexible workflows
- Consistent terminology used throughout ("source branch", "base branch")
- All documentation matches actual flexible branching behavior

This step ensures user-facing documentation matches the flexible branching implementation.

## Proposed Solution

After analyzing the codebase, I found references to "main branch" in several user-facing locations that need to be updated to reflect flexible base branch support:

### Changes Made:

1. **MCP Tool Descriptions Updated** ✅
   - Updated `/swissarmyhammer-tools/src/mcp/tools/issues/merge/description.md`
   - Changed "main branch" to "source branch" in description and return documentation

2. **CLI Help Text Updated** ✅
   - Updated `/swissarmyhammer-cli/src/cli.rs` line 254
   - Changed "Merge completed issue to main" to "Merge completed issue to source branch"

3. **Built-in Prompt Descriptions Updated** ✅
   - Updated `/builtin/prompts/issue/merge.md`
   - Changed description, goal, and process text from "main branch" to "source branch"

4. **Code Documentation Updated** ✅
   - Updated module documentation in `/swissarmyhammer-tools/src/mcp/tools/issues/mod.rs`
   - Updated git operations documentation in `/swissarmyhammer/src/git.rs`
   - Enhanced backward compatibility comments for clarity

### Files Identified but Appropriately Left Unchanged:

- Historical specifications in `/specification/complete/issues.md` - preserved for historical accuracy
- Issue files and specification documents - contain task descriptions, not user-facing text
- Backward compatibility code comments that accurately describe fallback behavior
- Git logs and coverage reports - generated content

### Terminology Consistency:

All user-facing text now consistently uses:
- "source branch" - for the branch that an issue was created from
- "base branch" - for general discussion of flexible branching
- Avoided "main branch" except where documenting backward compatibility behavior

The implementation ensures that tool descriptions, help text, prompts, and documentation all accurately reflect the flexible branching capabilities while maintaining backward compatibility.