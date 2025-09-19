# Remove CLI Command Timeouts (Redundant with Action Timeouts)

## Problem

The CLI commands currently accept timeout parameters in `swissarmyhammer-cli/src/cli.rs:262,279,371`:
- Flow run command: `timeout: Option<String>`
- Flow resume command: `timeout: Option<String>` 
- Flow test command: `timeout: Option<String>`

These CLI-level timeouts are redundant since workflows already have action-level timeouts that prevent hanging.

## Current CLI Timeout Usage

Found in:
- `swissarmyhammer-cli/src/commands/flow/run.rs:34` - `timeout: Option<String>`
- `swissarmyhammer-cli/src/commands/flow/resume.rs:16` - `timeout: Option<String>`
- `swissarmyhammer-cli/src/commands/flow/test.rs:12` - `timeout: Option<String>`
- CLI parsing and duration conversion logic
- Test utilities that specify timeouts via CLI

## Rationale for Removal

### Action Timeouts Provide Sufficient Protection
- With unified `action_timeout` (1 hour default), workflows cannot hang indefinitely
- Each action within the workflow has timeout protection
- MCP server timeout (15 minutes) provides additional safety net
- Multiple layers of timeout protection make CLI timeout redundant

### Simplifies CLI Interface
- Removes timeout parameter from all flow commands
- Eliminates need for duration parsing from CLI strings
- Cleaner command-line interface with fewer options
- Less cognitive overhead for users

### Eliminates Timeout Hierarchy Confusion
- No more conflicts between CLI timeout and action timeout
- Single point of timeout control at the action level
- More predictable behavior - workflows complete when actions complete/timeout
- Removes complexity of calculating appropriate CLI timeout values

## Implementation Tasks

### 1. Remove CLI Timeout Parameters
- Remove `timeout: Option<String>` from all flow command structs
- Remove timeout parsing logic from CLI command handlers
- Remove `parse_duration()` usage for CLI timeouts
- Update CLI argument parsing to remove timeout options

### 2. Remove Timeout Processing Logic
- Remove timeout duration calculation in `run.rs`, `resume.rs`, `test.rs`
- Remove `tokio::time::timeout` wrapper around workflow execution
- Remove timeout-specific error handling and messages
- Simplify workflow execution to rely on action timeouts only

### 3. Update CLI Help and Documentation
- Remove timeout options from CLI help text
- Update command documentation to remove timeout references
- Remove timeout examples from CLI usage documentation
- Update any tutorials showing CLI timeout usage

### 4. Update Tests and Test Utilities
- Remove timeout parameters from test utilities in `in_process_test_utils.rs`
- Update integration tests that specify CLI timeouts
- Remove timeout validation tests for CLI commands
- Ensure tests work properly without CLI timeout specification

### 5. Clean Up Context Variables
- Remove `_timeout_secs` context variable setup
- Remove timeout-related context passing to workflows
- Simplify workflow context initialization

## Benefits After Removal

- Cleaner CLI interface with fewer parameters
- Elimination of timeout calculation complexity
- Single timeout control point (action level)
- Reduced user confusion about which timeout applies
- Simplified test setup and execution
- More predictable workflow behavior

## Files to Update

- `swissarmyhammer-cli/src/cli.rs` - CLI argument definitions
- `swissarmyhammer-cli/src/commands/flow/run.rs` - Flow run command
- `swissarmyhammer-cli/src/commands/flow/resume.rs` - Flow resume command  
- `swissarmyhammer-cli/src/commands/flow/test.rs` - Flow test command
- `swissarmyhammer-cli/tests/in_process_test_utils.rs` - Test utilities
- All CLI integration tests that use timeout parameters
- CLI documentation and help text