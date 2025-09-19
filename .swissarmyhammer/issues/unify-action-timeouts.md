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
