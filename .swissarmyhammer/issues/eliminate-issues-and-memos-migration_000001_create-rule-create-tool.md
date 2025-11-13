# Step 1: Create rule_create MCP Tool

Refer to ideas/eliminate-issues-and-memos-migration.md

## Goal

Implement the `rule_create` MCP tool that allows creating project-specific rules from specifications.

## Context

This is the first step in eliminating the issues and memos systems. The `rule_create` tool will allow specifications to be converted into executable rules that define "what success looks like" for a project.

## Architecture

```mermaid
graph TD
    A[MCP Client] -->|rule_create call| B[CreateRuleTool]
    B -->|validates params| C[Validation Layer]
    C -->|writes file| D[.swissarmyhammer/rules/]
    D -->|subdirectories| E[category/rule.md]
```

## Tool Design

### Location
- `swissarmyhammer-tools/src/mcp/tools/rules/create/`
- Create `mod.rs` for implementation
- Create `description.md` for tool documentation

### Parameters

```rust
struct CreateRuleParams {
    name: String,        // e.g. "code-quality/no-global-state"
    content: String,     // The rule checking instructions (markdown)
    severity: String,    // "error" | "warning" | "info" | "hint"
    tags: Option<Vec<String>>, // Optional tags like ["maintainability", "testing"]
}
```

### Generated File Format

```yaml
---
severity: error
tags: ["maintainability", "testing"]
---

Check for global mutable state...
```

### Key Features

1. **Directory Support**: Name can include subdirectories (e.g., "code-quality/my-rule")
2. **Auto-create Directories**: Use mkdir -p behavior to create parent directories
3. **Minimal Frontmatter**: Only severity (required) and tags (optional)
4. **No Validation**: Just write the file, don't validate rule content
5. **Category from Path**: Category derived from directory structure, not stored in frontmatter

## Implementation Tasks

1. Create `swissarmyhammer-tools/src/mcp/tools/rules/create/mod.rs`
   - Implement `CreateRuleTool` struct
   - Implement `McpTool` trait
   - Parse parameters and validate
   - Handle directory creation
   - Write rule file with frontmatter

2. Create `swissarmyhammer-tools/src/mcp/tools/rules/create/description.md`
   - Document tool purpose
   - Describe parameters
   - Provide usage examples

3. Register tool in `swissarmyhammer-tools/src/mcp/tools/rules/mod.rs`
   - Add `pub mod create;`
   - Export `CreateRuleTool`
   - Register in tool registry

4. Write comprehensive tests
   - Test basic rule creation
   - Test subdirectory creation
   - Test with and without tags
   - Test all severity levels
   - Test error cases (invalid severity, empty content, etc.)

## Testing Checklist

- ✅ Rule created in correct location
- ✅ Frontmatter properly formatted
- ✅ Subdirectories created automatically
- ✅ Tags optional and properly serialized
- ✅ Content preserved exactly
- ✅ Invalid severity rejected
- ✅ Empty name rejected
- ✅ Empty content rejected

## Acceptance Criteria

- `rule_create` tool registered and callable via MCP
- Creates files in `.swissarmyhammer/rules/` with proper structure
- Supports subdirectory paths in rule names
- Generates minimal YAML frontmatter (severity + optional tags)
- All tests passing
- Build succeeds with no warnings

## Estimated Changes

~150-200 lines of code (tool implementation, tests, documentation)
