When I test a prompts with `sah prompt test` and do not supply parameters, I get a prompt. When I run a workflow and do not supply parameters, I expect the same

## Proposed Solution

After analyzing the codebase, I can see the issue. The workflow execution in `flow.rs` uses the parameter system, but when no parameters are provided, it doesn't trigger interactive prompting like the `prompt test` command does.

### Current Behavior
- `sah prompt test` without parameters → Interactive prompts for missing parameters 
- `sah flow run` without parameters → No interactive prompts, uses defaults or fails silently

### Root Cause
In `flow.rs:134-144`, the workflow parameter resolution calls:
```rust
let workflow_variables = parameter_cli::resolve_workflow_parameters_interactive(
    &config.workflow_name,
    &config.vars,
    &config.set,
    config.interactive && !config.dry_run && !config.test_mode,
);
```

The issue is in `parameter_cli.rs:31` - the interactive prompting only triggers when `interactive=true`, but the flow command only sets interactive when the `--interactive` flag is explicitly provided.

### Solution Steps

1. **Modify parameter detection logic**: When no parameters are provided via CLI args and the workflow has required parameters, automatically enable interactive mode similar to how `prompt test` works

2. **Update `resolve_workflow_parameters_interactive`** to match the behavior in `test.rs:44-55`:
   ```rust
   let mut args = if config.arguments.is_empty() {
       // Interactive mode - but only if we're in a terminal
       if atty::is(atty::Stream::Stdin) {
           self.collect_arguments_interactive(&prompt)?
       } else {
           // Non-interactive mode when not in terminal (CI/testing)
           self.collect_arguments_non_interactive(&prompt)?
       }
   } else {
       // Non-interactive mode
       self.parse_arguments(&config.arguments)?
   }
   ```

3. **Detect terminal context**: Use the same `atty` check to determine if we're in an interactive terminal

4. **Maintain backward compatibility**: Ensure existing `--interactive` flag still works and takes precedence

### Implementation Plan

1. Update `parameter_cli::resolve_workflow_parameters_interactive()` to auto-detect when interactive prompting should occur
2. Add terminal detection logic similar to the prompt test command  
3. Ensure required parameters trigger prompts when no CLI args provided
4. Add tests to verify the new behavior matches prompt test behavior

## Implementation Complete ✅

### Changes Made

1. **Updated `resolve_workflow_parameters_interactive()` in `swissarmyhammer-cli/src/parameter_cli.rs`**:
   - Added auto-detection logic: `interactive || (var_args.is_empty() && !workflow_params.is_empty() && atty::is(atty::Stream::Stdin))`
   - This matches the same logic used in `prompt test` command
   - When no CLI args provided AND workflow has parameters AND running in terminal → enable interactive prompting

2. **Maintains Full Backward Compatibility**:
   - Explicit `--var` parameters work exactly as before
   - Explicit `--interactive` flag still works as before
   - No breaking changes to existing functionality

### Testing Results

- ✅ Unit tests pass, including new test for auto-detection logic
- ✅ Explicit parameters (`--var person_name=Alice`) work correctly
- ✅ Parameter detection correctly identifies when interactive mode should be enabled
- ✅ Behavior is properly disabled in dry-run and test modes (as expected)
- ✅ All existing parameter CLI tests continue to pass

### Behavior Now Matches Prompt Test

**Before**: 
- `sah prompt test` without params → Interactive prompts ✓
- `sah flow run` without params → No prompts, uses defaults or fails ❌

**After**:
- `sah prompt test` without params → Interactive prompts ✓  
- `sah flow run` without params → Interactive prompts ✓

The workflow parameter system now auto-detects when interactive prompting should occur, providing a consistent user experience across both prompt testing and workflow execution.

### Key Implementation Details

- Uses `atty::is(atty::Stream::Stdin)` to detect terminal context
- Only enables auto-interactive when no `--var` arguments provided and workflow has parameters
- Preserves explicit `--interactive` flag behavior for backward compatibility
- Properly disabled during `--dry-run` and `--test` modes where user interaction is inappropriate