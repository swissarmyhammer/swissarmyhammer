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
