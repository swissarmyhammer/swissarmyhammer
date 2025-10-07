# MCP rules_check Tool Should Call swissarmyhammer-rules Directly, Not CLI

## Problem

The MCP `rules_check` tool in `swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs` currently shells out to the CLI command `sah rule check` instead of directly calling the `swissarmyhammer-rules` crate.

**Current implementation:**
- Lines 75-94: Builds a `Command` to execute `sah --format json rule check`
- Lines 110-151: Returns hardcoded empty values (empty violations array, 0 rules_checked, 0 files_checked) in all three code paths instead of parsing JSON output
- Result: Always returns "0 rules against 0 files" regardless of actual CLI output

## Why This Is Wrong

1. **Performance**: Spawning a subprocess is slow and wasteful
2. **Coupling**: Creates unnecessary dependency on CLI being built and in PATH
3. **Error-prone**: Requires parsing CLI output, adding serialization/deserialization overhead
4. **Maintenance**: Changes to CLI flags/output format can break the MCP tool
5. **Testing**: Harder to test, requires full CLI binary
6. **Currently broken**: The parsing code doesn't actually parse - it just returns hardcoded zeros
7. **Architecture violation**: MCP should not depend on CLI - both should independently consume the core library

## Correct Architecture

Both the CLI and MCP should be independent consumers of the `swissarmyhammer-rules` crate:

```
swissarmyhammer-rules (core library)
    ↑                    ↑
    |                    |
swissarmyhammer-cli   swissarmyhammer-tools/mcp
```

**Not this (current broken design):**

```
swissarmyhammer-rules
    ↑
    |
swissarmyhammer-cli
    ↑
    |
swissarmyhammer-tools/mcp  ← WRONG: MCP depends on CLI
```

## Solution

The MCP tool should directly use `swissarmyhammer-rules::RuleChecker`:

```rust
use swissarmyhammer_rules::{RuleChecker, RuleCheckRequest};

async fn execute_rule_check(&self, request: &RuleCheckRequest) -> Result<RuleCheckResponse> {
    let checker = RuleChecker::new()?;
    let result = checker.check_with_filters(request).await?;
    // Convert to MCP response format
    Ok(result)
}
```

## Benefits

- Direct library calls, no subprocess overhead
- Type-safe, no serialization needed
- Consistent behavior with CLI (both use same underlying code)
- Easier to test and maintain
- Actually works correctly
- Proper separation of concerns: core library, CLI interface, MCP interface

## Files Affected

- `swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs` - main implementation
- `swissarmyhammer-tools/Cargo.toml` - may need to add swissarmyhammer-rules dependency


## Proposed Solution

### Implementation Plan

1. **Add Dependencies**
   - Add `swissarmyhammer-rules` to Cargo.toml
   - Add `swissarmyhammer-workflow` to Cargo.toml (needed for AgentExecutor)
   - Note: Dependencies cannot be added due to circular dependency (see below)

2. **Refactor execute_rule_check method**
   - Remove CLI subprocess Command execution
   - Create and initialize RuleChecker with AgentExecutor
   - Call `checker.check_with_filters()` directly
   - Map the Result types from library to MCP response types

3. **Key Design Decisions**
   - Use `RuleChecker::check_with_filters()` as it provides the complete workflow
   - Need to create AgentExecutor for RuleChecker initialization
   - Map `RuleCheckRequest` from MCP to rules crate version
   - Map `RuleCheckResult` from rules crate to MCP `RuleCheckResponse`
   - Handle Result types: rules crate returns violations as errors, MCP returns them as success with violation data

4. **Agent Executor Setup**
   - Use `WorkflowTemplateContext::load_with_agent_config()` for configuration
   - Create `AgentExecutionContext` from workflow context
   - Use `AgentExecutorFactory::create_executor()` to get the executor
   - Initialize the executor before creating RuleChecker
   - Wrap in Arc for RuleChecker

5. **Error Handling Strategy**
   - Rules crate fails fast on Error violations (returns Err)
   - Non-error violations (Warning/Info/Hint) are logged but don't error
   - Need to catch violations from Err and convert to RuleCheckResponse with violations
   - Other errors (IO, config, etc.) should propagate as McpError

### Testing Strategy

- Write unit tests for the new direct integration
- Test that violations are properly returned
- Test that filters work correctly
- Ensure no CLI dependency remains


## Circular Dependency Problem Discovered

When attempting to add `swissarmyhammer-workflow` as a dependency to `swissarmyhammer-tools`, a circular dependency was discovered:

```
swissarmyhammer-workflow -> swissarmyhammer-tools -> swissarmyhammer-rules -> swissarmyhammer-workflow
```

The `swissarmyhammer-workflow` crate depends on `swissarmyhammer-tools` for the MCP server implementation.

### Root Cause

The `RuleChecker` in `swissarmyhammer-rules` requires an `AgentExecutor` (from `swissarmyhammer-workflow`) to be passed in at construction time. This means any code that wants to use `RuleChecker` must have access to the workflow crate to create the executor.

### Possible Solutions

1. **Extract Agent Interface**: Move the `AgentExecutor` trait to a separate crate (e.g., `swissarmyhammer-agent`) that both `workflow` and `rules` can depend on without creating a cycle.

2. **Lazy Initialization**: Modify `RuleChecker` to not require the agent at construction time, but instead accept it per-call or use dependency injection.

3. **Accept Current Architecture**: Keep the CLI-based approach since it already works and the architectural constraint makes direct library calls problematic.

4. **Move MCP Server**: Move the MCP server out of `swissarmyhammer-tools` into its own crate so that `tools` doesn't create the cycle.

### Recommended Solution

Option 1 (Extract Agent Interface) is the cleanest solution. It would:
- Create a new `swissarmyhammer-agent` crate with just the `AgentExecutor` trait
- Move agent implementations to this crate
- Allow `rules` to depend on `agent` without depending on `workflow`
- Allow `tools` to depend on both `agent` and `rules` without circular dependencies

This is a larger refactoring than initially anticipated.


## Investigation Complete - Architectural Refactoring Required

### Summary

After thorough investigation, I discovered that directly calling `swissarmyhammer-rules` from the MCP tool creates a **circular dependency** that prevents compilation:

```
swissarmyhammer-workflow 
  ↓ (depends on tools for MCP server)
swissarmyhammer-tools
  ↓ (would need to depend on workflow for AgentExecutor)  
swissarmyhammer-rules
  ↓ (depends on workflow for AgentExecutor trait)
swissarmyhammer-workflow  ← CYCLE!
```

### Why This Happens

1. `RuleChecker` requires an `AgentExecutor` instance at construction time
2. `AgentExecutor` trait is defined in `swissarmyhammer-workflow`
3. `swissarmyhammer-workflow` depends on `swissarmyhammer-tools` for the MCP server implementation
4. Therefore, `swissarmyhammer-tools` **cannot** depend on `swissarmyhammer-workflow`

### Why CLI Works But MCP Doesn't

- **CLI**: Can depend on both `workflow` and `rules` because `workflow` does NOT depend on `cli`
- **MCP Tools**: Cannot depend on `workflow` because `workflow` DOES depend on `tools` (for the MCP server)

### Recommended Solutions

#### Option 1: Extract Agent Interface (RECOMMENDED)
Create a new crate `swissarmyhammer-agent` containing:
- `AgentExecutor` trait
- `AgentExecutionContext`
- `AgentResponse` and related types
- Agent executor implementations

Dependency graph becomes:
```
swissarmyhammer-agent (new crate with trait + implementations)
    ↑                                    ↑
    |                                    |
swissarmyhammer-rules              swissarmyhammer-workflow
    ↑                                    ↑
    |                                    |
swissarmyhammer-tools  ←────────────────┘
```

**Benefits:**
- Clean separation of concerns
- No circular dependencies
- MCP tools can directly use `RuleChecker`
- Maintains proper layering

**Drawbacks:**
- Requires creating a new crate
- Need to move agent code from workflow to agent crate
- Multiple crates need updates

#### Option 2: Keep Current CLI-Based Approach
Accept that the MCP tool shells out to CLI as a pragmatic solution given architectural constraints.

**Benefits:**
- No refactoring needed
- Works today (though with hardcoded return values)
- Simple to understand

**Drawbacks:**
- Performance overhead of subprocess
- Dependency on CLI binary being built and in PATH
- Violates architectural principle of MCP/CLI both consuming core libraries independently
- Current implementation is broken (returns hardcoded zeros)

#### Option 3: Move MCP Server Out of Tools Crate
Create `swissarmyhammer-mcp-server` crate separate from `tools`.

**Benefits:**
- Breaks the cycle
- Tools could then depend on workflow

**Drawbacks:**
- Adds another crate
- Less clear than Option 1
- MCP server is conceptually part of tools

### Recommendation

**Implement Option 1** - Extract agent interface into its own crate. This is the cleanest architectural solution and aligns with proper dependency management principles. The refactoring cost is worth it for:
- Eliminating subprocess overhead
- Proper separation of concerns
- Type-safe direct library calls
- Easier testing and maintenance

### Current Status

- Implementation uses CLI-based approach
- Build passes successfully
- No circular dependencies present
- Awaiting decision on refactoring approach


## Code Review Changes

Applied code review feedback to improve issue documentation:

1. **Removed temporal language** - Eliminated temporal markers like "(DONE)" and "has been reverted" per coding standards. Issue now describes current state without referencing past actions.

2. **Standardized crate naming** - Changed all references from "swissarmyhammer-agent-interface" to "swissarmyhammer-agent" for consistency.

3. **Verified implementation claims** - Confirmed that the current MCP implementation (lines 110-151) returns hardcoded empty values in all three code paths. The claim is accurate.

4. **Verified line number references** - Confirmed that referenced line numbers (75-94, 110-151) are accurate to the current implementation.

All documentation now adheres to coding standards and accurately reflects the current state of the code.