No, do not combine the system prompt with the user prompt, you pass the entire system prompt to `--append-system-prompt`.

Third try -- THINK -- just get he system prompt and pass it to that switch. Don't make this hard.

## Proposed Solution

The issue is that we're currently combining the system prompt with the user prompt in the `prepare_final_prompt` method, but we should be using Claude Code's `--append-system-prompt` parameter instead.

### Current Implementation Problems:
1. The `prepare_final_prompt` method combines system prompt and user prompt into one string
2. This approach doesn't leverage Claude Code's dedicated `--append-system-prompt` parameter
3. The combined approach may not work as well as using the dedicated parameter

### Solution Steps:
1. **Modify `execute_claude_cli` method** to accept an optional system prompt parameter and use `--append-system-prompt` when available
2. **Update `prepare_final_prompt`** to return just the user prompt and separately return the system prompt 
3. **Update the `execute` method** to pass the system prompt separately to `execute_claude_cli`
4. **Test the integration** to ensure it works correctly

The key change is in `/Users/wballard/github/swissarmyhammer/swissarmyhammer/src/workflow/actions.rs` around lines 420-450 where `execute_claude_cli` is defined.

## Implementation Completed ✅

Successfully implemented the system prompt integration using Claude Code's `--append-system-prompt` parameter instead of combining prompts.

### Changes Made:

1. **Modified `execute_claude_cli` method** (`actions.rs:420`):
   - Added `system_prompt: Option<String>` parameter
   - Updated to use `--append-system-prompt` CLI argument when system prompt is provided
   - Maintained backwards compatibility with existing functionality

2. **Replaced `prepare_final_prompt` with `prepare_prompts`** (`actions.rs:369`):
   - New method returns `(String, Option<String>)` tuple for user prompt and system prompt
   - Separates system prompt rendering from user prompt processing
   - Preserves existing configuration and environment variable support

3. **Updated `execute` method** (`actions.rs:535`):
   - Modified to use new `prepare_prompts` method
   - Passes system prompt separately to `execute_claude_cli`
   - Maintains existing logging and error handling

### Testing Results:
- ✅ Code compiles successfully (`cargo build`)
- ✅ All tests pass (`cargo nextest run --fail-fast`)
- ✅ No clippy warnings (`cargo clippy`)
- ✅ Code formatted properly (`cargo fmt --all`)

### Code Review Completed:
- ✅ Implementation correctly uses `--append-system-prompt` parameter
- ✅ No lint warnings or errors found
- ✅ Proper separation of user and system prompts
- ✅ Maintains backwards compatibility and error handling
- ✅ All existing functionality preserved while improving Claude Code integration

The implementation now correctly uses Claude Code's dedicated `--append-system-prompt` parameter instead of combining the system prompt with the user prompt, addressing the issue requirement.