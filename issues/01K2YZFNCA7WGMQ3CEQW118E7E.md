we do not need --set and --var pick one and have a single consistent switch

## Proposed Solution

After analyzing the codebase, I found both `--set` and `--var` are used throughout the CLI for different purposes:

1. **`--var` (KEY=VALUE)**: Used for workflow variables and prompt arguments - the primary parameter passing mechanism
2. **`--set` (KEY=VALUE)**: Used specifically for liquid template variables in workflow actions

### Analysis of Current Usage:
- `--var` appears in: `prompt test`, `flow run`, `flow test`, `config test`
- `--set` appears in: `prompt test`, `flow run`, `flow test`

### Decision: Keep `--var` and remove `--set`

**Rationale:**
- `--var` is more widely used across different command contexts
- The distinction between workflow variables and template variables is confusing to users
- A single parameter mechanism is simpler and more consistent
- Most examples in the codebase already use `--var`

### Implementation Steps:
1. Remove all `--set` parameters from CLI definitions
2. Update help text and examples to use only `--var` 
3. Update parameter processing logic to handle template variables through `--var`
4. Run tests to ensure backward compatibility

This consolidation will provide a single, consistent parameter passing mechanism: `--var key=value`.

## Implementation Completed

Successfully consolidated the CLI parameter handling to use only `--var` instead of both `--set` and `--var`.

### Changes Made:

1. **CLI Definitions**: Removed all `--set` parameter definitions from CLI structs:
   - `PromptSubcommand::Test` 
   - `FlowSubcommand::Run`
   - `FlowSubcommand::Test`

2. **Function Signatures**: Updated `resolve_workflow_parameters_interactive()` to remove the unused `_set_args` parameter

3. **Pattern Matching**: Removed all `set,` fields from pattern matches throughout the codebase

4. **Template Variable Logic**: Simplified template variable handling - all variables now go through the regular workflow variables system via `--var`

5. **Help Text**: Updated all examples in CLI help to use `--var` instead of `--set`:
   - `swissarmyhammer prompt test code-review --var author=John --var version=1.0`
   - `swissarmyhammer flow test greeting --var name=John --var language=Spanish`

6. **Tests**: Updated all test cases to use `--var` instead of `--set` and removed obsolete tests for `--set` functionality

### Result:
- **Single Parameter Mechanism**: All variables now use `--var key=value`
- **Simplified User Experience**: No more confusion between `--set` and `--var`
- **Consistent API**: Same parameter passing approach across all commands
- **Backward Compatibility**: Existing `--var` usage continues to work exactly as before

All tests pass and the build succeeds. The CLI now has a single, consistent parameter passing mechanism.