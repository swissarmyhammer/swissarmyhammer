# Update Built-in Prompts to Use New Abort MCP Tool

Refer to ./specification/abort.md

## Objective
Update the built-in abort prompt and any other prompts that reference abort functionality to use the new MCP abort tool instead of the old "ABORT ERROR" string response pattern.

## Context
The current built-in prompt `builtin/prompts/abort.md` instructs users to "Respond only with ABORT ERROR". This needs to be updated to use the new MCP abort tool, providing clear instructions for proper abort tool usage.

## Tasks

### 1. Update builtin/prompts/abort.md
Location: `builtin/prompts/abort.md:9`
- Replace current "Respond only with ABORT ERROR" instruction
- Add clear instructions for using the abort MCP tool
- Include example tool usage with parameters
- Maintain prompt purpose and context

### 2. Search for Other Abort References
- Search all builtin prompts for references to "ABORT ERROR" strings
- Search for references to abort functionality
- Update any prompts that mention or use abort patterns

### 3. Improve Abort Tool Documentation
- Ensure abort tool usage instructions are clear and actionable
- Provide examples of when and how to use the abort tool
- Include parameter descriptions and expected behavior

## Implementation Details

### Updated Abort Prompt Content
```markdown
# Abort Prompt

When you need to abort or terminate the current workflow, action, or process, use the abort MCP tool instead of continuing.

## Usage

Use the `abort` tool with a descriptive reason:

```json
{
  "tool": "abort",
  "parameters": {
    "reason": "Clear description of why the abort was necessary"
  }
}
```

## Examples

Abort due to user cancellation:
```json
{
  "tool": "abort", 
  "parameters": {
    "reason": "User cancelled the destructive operation"
  }
}
```

Abort due to unsafe conditions:
```json
{
  "tool": "abort",
  "parameters": {
    "reason": "Detected potentially unsafe file system operation"
  }
}
```

## When to Abort

- User explicitly requests cancellation
- Unsafe or potentially destructive operations detected
- Prerequisites or requirements cannot be met
- System is in an inconsistent state

The abort tool will immediately terminate the current workflow and provide the reason to help with debugging and understanding the termination.
```

### Prompt Search Strategy
Use grep/search tools to find:
- References to "ABORT ERROR" in builtin prompts
- References to abort functionality or termination patterns
- Any other prompts that might need updating

## Validation Criteria
- [ ] `builtin/prompts/abort.md` uses new MCP tool instructions
- [ ] No references to "ABORT ERROR" string patterns remain
- [ ] Tool usage examples are clear and actionable
- [ ] Abort tool parameter requirements are documented
- [ ] Other prompts using abort patterns are updated
- [ ] Documentation is consistent with MCP tool implementation

## Testing Requirements
- Test prompt rendering with new abort tool instructions
- Validate that prompts produce correct tool usage
- Ensure prompt changes don't break existing workflows
- Test abort tool usage in various scenarios

## Files to Modify
- `builtin/prompts/abort.md` - Primary abort prompt
- Any other builtin prompts with abort references (to be identified)

## Dependencies
- ABORT_000260_core-abort-tool-implementation (abort tool must be functional)

## Follow-up Issues
- ABORT_000265_comprehensive-testing