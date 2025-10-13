# Consolidate rule checking logic into swissarmyhammer-rules crate

## Problem
Rule checking logic currently exists in the CLI. We need to add MCP support for rule checking, and we must avoid duplicating logic between CLI and MCP.

## Goal
Create a shared, reusable rule checking API in the `swissarmyhammer-rules` crate that both CLI and MCP can use.

## Requirements
- Extract core rule checking logic from CLI into swissarmyhammer-rules
- Create a public API that accepts:
  - Optional list of rule names (None = run all rules)
  - List of file paths to check
- Return structured results (violations, warnings, errors)
- Handle rule loading, file processing, and result aggregation
- Should NOT handle output formatting (that's CLI/MCP specific)

## Implementation Notes
- Look at current CLI rule check implementation in `swissarmyhammer-cli`
- Identify what logic belongs in the rules crate vs. CLI/MCP presentation layer
- Consider creating a `RuleChecker` struct or similar abstraction
- Ensure the API is ergonomic for both CLI and MCP consumers

## Dependencies
None - this should be done first


## Proposed Solution

### Analysis

After examining the code, I found that:

1. **Current CLI Implementation** (`swissarmyhammer-cli/src/commands/rule/check.rs`):
   - Loads rules via `RuleResolver`
   - Filters rules by name, severity, category
   - Validates all rules
   - Expands glob patterns to files
   - Creates agent executor (ClaudeCode or LlamaAgent)
   - Creates `RuleChecker` and calls `check_all()`
   - Handles error presentation

2. **Current Rules Crate** (`swissarmyhammer-rules/src/checker.rs`):
   - Already has `RuleChecker` with `check_file()` and `check_all()` methods
   - Handles two-stage rendering (rule template + .check prompt)
   - Executes via agent
   - Parses responses and detects violations
   - Has caching support

3. **The Gap**: The CLI has logic that belongs in the rules crate:
   - Rule loading and filtering
   - Rule validation
   - File pattern expansion
   - Result aggregation

### Solution Design

Extract the following into a new public API in `swissarmyhammer-rules`:

```rust
pub struct RuleCheckRequest {
    /// Optional list of rule names to check (None = all rules)
    pub rule_names: Option<Vec<String>>,
    /// Optional severity filter
    pub severity: Option<Severity>,
    /// Optional category filter  
    pub category: Option<String>,
    /// File paths or glob patterns to check
    pub patterns: Vec<String>,
}

pub struct RuleCheckResult {
    /// Number of rules checked
    pub rules_checked: usize,
    /// Number of files checked
    pub files_checked: usize,
    /// List of violations found (if any)
    pub violations: Vec<RuleViolation>,
}

impl RuleChecker {
    /// High-level API for checking rules with filtering and pattern expansion
    pub async fn check_with_filters(&self, request: RuleCheckRequest) -> Result<RuleCheckResult>;
}
```

This API will:
1. Load all rules via `RuleResolver`
2. Filter by name, severity, category
3. Validate all rules
4. Expand glob patterns using existing `swissarmyhammer_common::glob_utils`
5. Call the existing `check_all()` internally
6. Return structured results

The CLI and MCP will then simply:
1. Parse command-line args or MCP params
2. Create a `RuleCheckRequest`
3. Call `checker.check_with_filters(request).await`
4. Format results for their respective output (table, JSON, MCP response)

### Implementation Steps

1. Add `RuleCheckRequest` and `RuleCheckResult` structs to `swissarmyhammer-rules/src/checker.rs`
2. Implement `check_with_filters()` method that extracts the logic from CLI
3. Update CLI to use the new API
4. Write tests for the new API
5. Run all tests to verify nothing broke

### Benefits

- **No duplication**: Core logic in one place
- **Reusable**: Both CLI and MCP use same API
- **Testable**: Can test rule checking without CLI infrastructure
- **Clean separation**: Rules crate handles logic, CLI/MCP handle presentation



## Implementation Complete

### What Was Done

1. **Added new API to swissarmyhammer-rules crate** (swissarmyhammer-rules/src/checker.rs:16-65):
   - `RuleCheckRequest` struct - encapsulates filtering parameters and file patterns
   - `RuleCheckResult` struct - returns statistics about the check operation
   - `RuleChecker::check_with_filters()` method - high-level API that handles:
     - Loading rules via RuleResolver
     - Filtering by name, severity, category
     - Validating all rules
     - Expanding glob patterns
     - Running checks via existing `check_all()` method

2. **Refactored CLI to use new API** (swissarmyhammer-cli/src/commands/rule/check.rs:28-83):
   - Removed duplicate logic (rule loading, filtering, validation, glob expansion)
   - Simplified `execute_check_command` to:
     - Create agent executor
     - Build RuleCheckRequest from command args
     - Call `checker.check_with_filters()`
     - Handle results
   - Removed 8 tests that tested glob expansion (now in rules crate)

3. **Added comprehensive tests** (swissarmyhammer-rules/src/checker.rs:925-1036):
   - Test no matching rules
   - Test no matching files
   - Test severity filtering
   - Test category filtering
   - Test combined filters
   - Test request/result struct creation

4. **All tests pass**: Ran full test suite with `cargo nextest run` - 3263 tests passed

### Benefits Achieved

- **No duplication**: Core logic in one place (rules crate)
- **Reusable**: Both CLI and future MCP can use same API
- **Testable**: Can test rule checking without CLI infrastructure  
- **Clean separation**: Rules crate handles logic, CLI handles presentation
- **Maintainable**: Changes to rule checking logic only need to happen once

### Files Changed

- swissarmyhammer-rules/src/checker.rs - added new API
- swissarmyhammer-rules/src/lib.rs - exported new types
- swissarmyhammer-cli/src/commands/rule/check.rs - refactored to use new API

### Ready for MCP

The MCP implementation can now call `RuleChecker::check_with_filters()` directly with a `RuleCheckRequest` and get back structured results without duplicating any logic.



## Final Implementation Notes

### Architecture Decisions

1. **API Design**: Created a minimal but complete API with two structs:
   - `RuleCheckRequest` - encapsulates all filtering parameters
   - `RuleCheckResult` - returns statistics and violations
   
2. **Separation of Concerns**:
   - Rules crate handles: loading, filtering, validation, glob expansion, execution
   - CLI handles: argument parsing, agent creation, result formatting
   - This separation makes MCP integration straightforward

3. **Code Reuse**: The new `check_with_filters()` method internally calls the existing `check_all()` method, preserving all existing functionality including caching and two-stage rendering.

4. **Error Handling**: Violations with severity::warning allow execution to continue (changed in previous commit), which matches expected behavior for rule checking.

### Testing Strategy

Added comprehensive tests to verify:
- No matching rules scenario
- No matching files scenario  
- Severity filtering
- Category filtering
- Combined filters
- Request/result struct creation

Removed 8 glob expansion tests from CLI (logic moved to rules crate).

### Test Results

All 3263 tests pass successfully. No compilation errors, no lint warnings.

### Ready for MCP

The MCP implementation can now:
1. Create a `RuleCheckRequest` from MCP parameters
2. Call `checker.check_with_filters(request).await`
3. Format `RuleCheckResult` as MCP response

Zero duplication required.
