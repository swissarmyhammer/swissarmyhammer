# Unify Action Timeouts into Single Configuration

## Problem

Currently, workflow actions have multiple separate timeout configurations in `swissarmyhammer-workflow/src/actions.rs:111-117`:
- `prompt_timeout`: For prompt actions
- `user_input_timeout`: For user input actions
- `sub_workflow_timeout`: For sub-workflow actions

This creates unnecessary complexity when a single timeout value would work for all action types.

## Solution

### Replace Multiple Timeouts with Single `action_timeout`
- Remove `prompt_timeout`, `user_input_timeout`, and `sub_workflow_timeout` from `ActionTimeouts` struct
- Add single `action_timeout: Duration` field
- Set default value to 1 hour (3600 seconds)

### Update All Action Types
- Update `PromptAction` to use unified timeout
- Update `WaitAction` (user input) to use unified timeout
- Update `SubWorkflowAction` to use unified timeout
- Any other actions using these separate timeouts

### Benefits
- Simplified configuration - one timeout to rule them all
- Reduced cognitive overhead for users
- Consistent timeout behavior across all actions
- Still allows per-action timeout overrides via `with_timeout()` methods

## Implementation Details

### Before (current)
```rust
pub struct ActionTimeouts {
    pub prompt_timeout: Duration,
    pub user_input_timeout: Duration,
    pub sub_workflow_timeout: Duration,
}

impl Default for ActionTimeouts {
    fn default() -> Self {
        Self {
            prompt_timeout: Duration::from_secs(env_var_or_default("SAH_PROMPT_TIMEOUT_SECONDS", 300)),
            user_input_timeout: Duration::from_secs(env_var_or_default("SAH_USER_INPUT_TIMEOUT_SECONDS", 300)),
            sub_workflow_timeout: Duration::from_secs(env_var_or_default("SAH_SUB_WORKFLOW_TIMEOUT_SECONDS", 900)),
        }
    }
}
```

### After (proposed)
```rust
pub struct ActionTimeouts {
    pub action_timeout: Duration,
}

impl Default for ActionTimeouts {
    fn default() -> Self {
        Self {
            action_timeout: Duration::from_secs(3600), // 1 hour
        }
    }
}
```

### Update Action Constructors
- `PromptAction::new()` uses `timeouts.action_timeout`
- `WaitAction` uses `timeouts.action_timeout`
- `SubWorkflowAction::new()` uses `timeouts.action_timeout`

## Files to Update

- `swissarmyhammer-workflow/src/actions.rs` - Main timeout struct and action implementations
- Any tests that reference the old timeout field names
- Documentation mentioning the separate timeout types
- Configuration examples that set individual timeout values

## Migration Notes

- This is a breaking change to the `ActionTimeouts` API. No backward compatibility.
- Existing code using separate timeouts will need to be updated
- Any environment variables for separate timeouts should be removed
- Update any YAML workflow configurations that might reference separate timeout settings


## Proposed Solution

Based on code analysis of `swissarmyhammer-workflow/src/actions.rs`, I found the current implementation:

### Current State Analysis
- **ActionTimeouts struct** (lines 111-117): Has 3 separate timeout fields
- **Environment variables**: Uses different env vars with different defaults:
  - `prompt_timeout`: 8 hours (28,800 seconds) - env: `SWISSARMYHAMMER_PROMPT_TIMEOUT`
  - `user_input_timeout`: 5 minutes (300 seconds) - env: `SWISSARMYHAMMER_USER_INPUT_TIMEOUT` 
  - `sub_workflow_timeout`: 24 hours (86,400 seconds) - env: `SWISSARMYHAMMER_SUB_WORKFLOW_TIMEOUT`

### Usage Locations Found
- `prompt_timeout` used in: lines 658, 1382, 2594
- `user_input_timeout` used in: lines 1005, 1014
- `sub_workflow_timeout` used in: line 1291

### Implementation Steps

1. **Replace ActionTimeouts struct** with single `action_timeout` field (1 hour default)
2. **Update all 6 usage locations** to use `timeouts.action_timeout`
3. **Update tests** that reference the old timeout field names (line 2594)
4. **Remove environment variable support** for the 3 separate timeout types
5. **Verify no other files** use the old timeout field names

### Default Timeout Decision
I'll use **1 hour (3600 seconds)** as the default, which is a reasonable middle ground between the current values and provides sufficient time for most actions while preventing indefinite hangs.

### Code Changes Required
```rust
// Before
pub struct ActionTimeouts {
    pub prompt_timeout: Duration,
    pub user_input_timeout: Duration, 
    pub sub_workflow_timeout: Duration,
}

// After  
pub struct ActionTimeouts {
    pub action_timeout: Duration,
}
```
## Implementation Completed

Successfully implemented the unified action timeout solution. All changes compile and pass tests.

### Changes Made

1. **Updated ActionTimeouts struct** in `swissarmyhammer-workflow/src/actions.rs:111-118`:
   - Removed 3 separate timeout fields
   - Added single `action_timeout: Duration` field
   - Set default to 3600 seconds (1 hour)
   - Removed environment variable support for separate timeouts

2. **Updated all 6 usage locations** to use `timeouts.action_timeout`:
   - PromptAction (line 658)
   - WaitAction user input (lines 1005, 1014) 
   - SubWorkflowAction (line 1291)
   - Default timeout fallback (line 1382)
   - Test assertion (line 2594)

3. **Updated 4 test files** to use the new field:
   - `actions_tests/prompt_action_tests.rs`
   - `actions_tests/sub_workflow_action_tests.rs` 
   - `actions_tests/shell_action_tests.rs` (2 locations)

### Verification

- ✅ Code compiles successfully
- ✅ All 519 unit tests pass
- ✅ No references to old timeout fields remain in codebase

### Result

The ActionTimeouts API is now simplified to a single `action_timeout` field, providing consistent timeout behavior across all action types while maintaining the ability to override timeouts per-action using existing `with_timeout()` methods.