# Prompts with Parameters Not Working in Claude Code via MCP

## Problem

Prompts that define parameters (like the `plan` prompt) were not receiving parameter values when invoked through Claude Code's MCP integration. This was because the SwissArmyHammer MCP server was not exposing prompt parameters to Claude Code.

## Status

**FIXED** - The MCP server now properly exposes prompt parameters.

## Affected Prompts

Any prompt with parameters defined in frontmatter, including:

- `builtin/prompts/plan.md` - Requires `plan_filename` parameter
- Any other prompt with `parameters:` section in YAML frontmatter

## Example

The `plan` prompt defines:

```yaml
---
title: plan
description: Generate a step by step development plan from specification(s).
parameters:
  - name: plan_filename
    description: Path to the specific plan markdown file to process (optional)
    required: true
---
```

And uses it in the template:

```markdown
Process the specific plan file: {{ plan_filename }}
```

## Root Cause

The MCP server implementation in `swissarmyhammer-tools/src/mcp/server.rs` was not converting SwissArmyHammer prompt parameters to MCP `PromptArgument` structures when listing prompts.

**Before (line 654-655):**
```rust
// Domain prompts don't have parameters yet - using empty list for now
let arguments = None;
```

This meant that when Claude Code called `list_prompts()`, it received prompt definitions without any parameter information, so it couldn't know what parameters to collect from the user or pass to the prompt.

## The Fix

Modified `list_prompts()` in `swissarmyhammer-tools/src/mcp/server.rs` to properly convert SwissArmyHammer `Parameter` objects to MCP `PromptArgument` objects:

```rust
// Convert SwissArmyHammer prompt parameters to MCP PromptArguments
let arguments = if p.parameters.is_empty() {
    None
} else {
    Some(
        p.parameters
            .iter()
            .map(|param| PromptArgument {
                name: param.name.clone(),
                title: None,
                description: Some(param.description.clone()),
                required: Some(param.required),
            })
            .collect(),
    )
};
```

This ensures that:
1. Claude Code sees the parameters when listing prompts
2. Claude Code can prompt the user for required parameters
3. Parameters are properly passed when `get_prompt()` is called

## MCP Protocol Background

According to the MCP specification, prompts can have arguments defined as `PromptArgument` with:
- `name: String` - The parameter name
- `title: Option<String>` - Human-readable title
- `description: Option<String>` - Description of what the parameter is for
- `required: Option<bool>` - Whether the parameter is required

## Files Changed

- `swissarmyhammer-tools/src/mcp/server.rs` (lines 654-669)

## Testing

- ✅ All existing MCP tests pass
- ✅ Code compiles successfully
- ✅ `test_mcp_server_list_prompts` passes

## What Was Not the Issue

Initially investigated whether this was Claude Code issue #2089 (MCP tool parameters not being passed), but that issue affects **tools**, not **prompts**. MCP prompts and tools use different mechanisms:

- **Tools**: Parameters sent in `tools/call` JSON-RPC method
- **Prompts**: Parameters defined in `list_prompts` response and passed to `get_prompt` request

## Next Steps

The fix has been applied. Claude Code should now be able to:
1. See parameters when listing prompts via MCP
2. Prompt users for required parameters
3. Pass those parameters when calling `get_prompt`

## References

- MCP Specification: Model Context Protocol
- rmcp crate: https://docs.rs/rmcp/
- SwissArmyHammer prompt system: `swissarmyhammer-prompts/src/prompts.rs`
