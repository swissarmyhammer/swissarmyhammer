system prompt injection will always be enabled, and never turned off
## Proposed Solution

After analyzing the codebase, I found that system prompt injection can currently be disabled through:

1. **Environment variable**: `SAH_CLAUDE_SYSTEM_PROMPT_ENABLED=false` 
2. **Context parameter**: `claude_system_prompt_enabled` in workflow context

**Current behavior** (in `workflow/actions.rs:377-387`):
```rust
let enable_system_prompt = context
    .get("claude_system_prompt_enabled")
    .unwrap_or_default()
    .to_string()
    .parse()
    .unwrap_or_else(|_| {
        std::env::var("SAH_CLAUDE_SYSTEM_PROMPT_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true)
    });
```

**Required changes**:

1. **Remove environment variable check**: Remove `SAH_CLAUDE_SYSTEM_PROMPT_ENABLED` environment variable support
2. **Remove context parameter check**: Remove `claude_system_prompt_enabled` context parameter support  
3. **Force system prompt injection**: Always enable system prompt injection regardless of any configuration
4. **Update documentation**: Remove references to disabling system prompt injection
5. **Update tests**: Ensure tests verify system prompt is always enabled

**Files to modify**:
- `swissarmyhammer/src/workflow/actions.rs` - Remove conditional checks
- `doc/src/prompts.md` - Remove disable instructions
- `doc/src/architecture.md` - Update documentation
- `tests/system_prompt_integration_tests.rs` - Update tests

## Implementation Complete

I have successfully implemented the changes to ensure system prompt injection is always enabled and can never be turned off.

### Changes Made

1. **Modified `workflow/actions.rs`**:
   - Removed conditional checks for `claude_system_prompt_enabled` context parameter
   - Removed conditional checks for `SAH_CLAUDE_SYSTEM_PROMPT_ENABLED` environment variable
   - System prompt injection now always occurs regardless of any configuration
   - Updated function signature to ignore the context parameter for enabling/disabling

2. **Updated Documentation**:
   - `doc/src/prompts.md`: Removed instructions for disabling system prompt injection
   - `doc/src/architecture.md`: Updated flowchart and configuration documentation
   - `doc/src/rust-api.md`: Updated configuration example

3. **Updated Tests**:
   - `tests/system_prompt_integration_tests.rs`: Updated to test that system prompt is always enabled
   - Added test logic to verify environment variables have no effect on system prompt injection

### Files Modified

- `swissarmyhammer/src/workflow/actions.rs` - Core logic changes
- `doc/src/prompts.md` - Documentation updates
- `doc/src/architecture.md` - Documentation and flowchart updates
- `doc/src/rust-api.md` - API example updates
- `tests/system_prompt_integration_tests.rs` - Test updates

### Verification

- ✅ Code compiles successfully (`cargo build`)
- ✅ No linting warnings (`cargo clippy`)
- ✅ Code formatting applied (`cargo fmt --all`)
- ✅ All existing tests pass (`cargo test`)

### Behavior Change

**Before**: System prompt injection could be disabled via:
- Environment variable: `SAH_CLAUDE_SYSTEM_PROMPT_ENABLED=false`
- Context parameter: `claude_system_prompt_enabled: false`

**After**: System prompt injection is always enabled:
- Environment variables have no effect
- Context parameters have no effect
- System prompt is always attempted to be injected (if available)
- Only fails silently if system prompt file is missing or invalid