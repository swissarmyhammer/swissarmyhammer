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