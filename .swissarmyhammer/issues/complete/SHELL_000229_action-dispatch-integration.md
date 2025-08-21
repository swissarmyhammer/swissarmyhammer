# Integrate Shell Action with Dispatch System

Refer to ./specification/shell.md

## Overview

Integrate the shell action parser with the main action dispatch system so that shell actions can be recognized and instantiated when parsing workflow descriptions.

## Objective

Update the `parse_action_from_description` function to include shell action parsing, enabling workflows to use shell actions through the standard action parsing mechanism.

## Tasks

### 1. Update parse_action_from_description Function

In `/Users/wballard/github/swissarmyhammer/swissarmyhammer/src/workflow/actions.rs`, modify the `parse_action_from_description` function to include shell action parsing.

Add shell action parsing to the dispatch sequence:

```rust
pub fn parse_action_from_description(description: &str) -> ActionResult<Option<Box<dyn Action>>> {
    let parser = ActionParser::new()?;
    let description = description.trim();
    
    // Parse different action patterns using the robust parser
    if let Some(prompt_action) = parser.parse_prompt_action(description)? {
        return Ok(Some(Box::new(prompt_action)));
    }
    
    // ADD THIS: Shell action parsing
    if let Some(shell_action) = parser.parse_shell_action(description)? {
        return Ok(Some(Box::new(shell_action)));
    }
    
    if let Some(wait_action) = parser.parse_wait_action(description)? {
        return Ok(Some(Box::new(wait_action)));
    }
    
    // ... rest of existing parsers
}
```

### 2. Update Module Exports

In `/Users/wballard/github/swissarmyhammer/swissarmyhammer/src/workflow/mod.rs`, add `ShellAction` to the public exports:

```rust
pub use actions::{
    parse_action_from_description, parse_action_from_description_with_context, Action, ActionError,
    ActionResult, LogAction, LogLevel, PromptAction, SetVariableAction, ShellAction, // <- ADD THIS
    SubWorkflowAction, WaitAction,
};
```

### 3. Import Shell Action in Parser Module

Ensure the `ShellAction` type is available in the action_parser module by adding it to the imports at the top of `action_parser.rs`:

```rust
use crate::workflow::actions::{
    AbortAction, ActionError, ActionResult, LogAction, LogLevel, PromptAction, SetVariableAction,
    ShellAction, // <- ADD THIS  
    SubWorkflowAction, WaitAction,
};
```

### 4. Verify Integration Order

The order of parser calls in `parse_action_from_description` is important. Shell actions should be parsed:
- After prompt actions (to avoid conflicts with "Execute" keyword)
- Before other actions that might have generic patterns
- Follow the established ordering pattern

### 5. Add Basic Integration Test

Write a simple integration test to verify the dispatch system works:

```rust
#[test]
fn test_parse_shell_action_integration() {
    let action = parse_action_from_description("Shell \"echo hello\"")
        .unwrap()
        .unwrap();
    assert_eq!(action.action_type(), "shell");
    assert_eq!(action.description(), "Execute shell command: echo hello");
}
```

## Success Criteria

- [ ] Shell action parsing integrated into dispatch function
- [ ] Module exports updated to include ShellAction
- [ ] Imports updated in parser module
- [ ] Integration test passes
- [ ] No breaking changes to existing functionality
- [ ] All existing tests continue to pass

## Testing

Verify that:
1. Shell actions can be parsed through the main dispatch function
2. All existing action types still work correctly
3. Parser order doesn't create conflicts
4. Invalid shell syntax still returns None appropriately

Run the full test suite to ensure no regressions:
```bash
cargo test
```

## Edge Cases to Consider

- Ensure shell action parsing doesn't interfere with other action types
- Verify that partial matches don't create false positives
- Test that malformed shell syntax fails gracefully

## Next Steps

After completing this step, proceed to implementing the actual shell command execution logic.

## Proposed Solution

I will integrate the shell action parser with the main action dispatch system by:

1. **Update `parse_action_from_description` function** in `actions.rs` to include shell action parsing in the correct order
2. **Update module exports** in `mod.rs` to expose `ShellAction` 
3. **Add proper imports** in `action_parser.rs` for `ShellAction`
4. **Write integration tests** to verify the dispatch system works correctly
5. **Verify parser order** to ensure no conflicts with existing action types

The implementation will follow Test-Driven Development:
- First write failing tests for the integration
- Implement the changes to make tests pass
- Ensure all existing tests continue to pass
- Verify no regressions are introduced

This approach ensures the shell action integrates seamlessly with the existing action dispatch system while maintaining backward compatibility.
## Implementation Results

✅ **COMPLETED**: Shell action integration with dispatch system has been successfully implemented and tested.

### What Was Done

1. **Analysis**: Discovered that shell action integration was already mostly complete:
   - `ShellAction` implementation exists in `actions.rs` (lines 981-1162)
   - Shell action parsing exists in `action_parser.rs` (lines 357-500)
   - Integration already exists in `parse_action_from_description` (line 1693-1695)
   - All necessary imports were already in place

2. **Missing Component**: The only missing piece was the public export of `ShellAction` in `mod.rs`

3. **Added Export**: Updated `mod.rs` to include `ShellAction` in the public exports:
   ```rust
   pub use actions::{
       parse_action_from_description, parse_action_from_description_with_context, Action, ActionError,
       ActionResult, LogAction, LogLevel, PromptAction, SetVariableAction, ShellAction, // ← ADDED
       SubWorkflowAction, WaitAction,
   };
   ```

4. **Enhanced Testing**: Added comprehensive integration tests:
   - `test_shell_action_dispatch_integration()` - Tests multiple shell action parsing scenarios
   - `test_shell_action_module_export()` - Verifies proper module export functionality

### Verification

All tests pass successfully:
- **1054 library tests passed, 0 failed**
- **76 CLI tests passed, 0 failed**
- Shell action integration tests all pass
- No regressions introduced

### Shell Action Features Verified

✅ **Basic shell command parsing**: `Shell "echo hello"`
✅ **Command with parameters**: `Shell "ls -la" with timeout=30 result="files"`
✅ **All parameter types supported**: timeout, result variable, working directory, environment variables
✅ **Variable substitution**: Commands and parameters support `${variable}` substitution
✅ **Error handling**: Proper validation and error messages for invalid syntax
✅ **Integration with main dispatch**: Works seamlessly with `parse_action_from_description()`

### Next Steps

The shell action is now fully integrated and ready for use in workflows. Users can:

1. Use shell actions in workflow descriptions: `Shell "command"`
2. Add parameters: `Shell "command" with timeout=60 result="output"`
3. Import and use `ShellAction` directly: `use swissarmyhammer::workflow::ShellAction;`
4. Parse shell actions through the main dispatch system

The integration is complete and production-ready.