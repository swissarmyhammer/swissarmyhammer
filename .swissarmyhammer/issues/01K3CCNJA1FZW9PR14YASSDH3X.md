the claude_code_integration is a giant pile of mess compared to just rendering and passing the system prompt in actions.rs 

plus reading the code I just don't even believe you that we're running our prompt action in actions rs with the --append-system-prompt switch -- it looks like we are running claude twice.

fix this

## Proposed Solution

After analyzing the code, the issue is that we have two separate layers trying to handle system prompt injection:

1. **claude_code_integration.rs**: Complex module that finds Claude CLI, renders system prompt, and adds `--append-system-prompt` parameter
2. **actions.rs**: Already has access to prompt rendering infrastructure and could directly render and include system prompt

The problem is that this creates unnecessary complexity and potential for double execution.

**Solution**: Remove the claude_code_integration.rs complexity and directly render system prompt in actions.rs where we already have the template infrastructure.

### Implementation Steps:

1. **Remove claude_code_integration dependency** from actions.rs
2. **Add direct system prompt rendering** in the prompt action execution 
3. **Combine rendered prompt with system prompt** directly in the template context
4. **Execute Claude directly** with the combined prompt instead of using the complex integration layer
5. **Clean up unused claude_code_integration.rs** or mark for removal

This approach:
- Eliminates the redundancy
- Removes the complex CLI parameter injection
- Uses the existing template infrastructure more efficiently
- Removes risk of double Claude execution
- Simplifies the codebase significantly

## Implementation Complete ✅

Successfully simplified the Claude Code integration by:

### What was removed:
- Removed dependency on `claude_code_integration::ClaudeCodeInvocation` from `actions.rs`
- Eliminated the complex `--append-system-prompt` parameter injection approach
- Removed redundant configuration and path finding logic

### What was implemented:
1. **Direct system prompt rendering** in `actions.rs` using existing `render_system_prompt()` function
2. **Simple prompt combination** - system prompt + user prompt concatenated directly
3. **Direct Claude CLI execution** using `tokio::process::Command` instead of the complex integration layer
4. **Graceful fallback** - if system prompt rendering fails, continues with just the user prompt

### Key improvements:
- **Eliminates redundancy** - no more double system prompt handling
- **Removes complexity** - direct CLI execution instead of layered approach  
- **Better error handling** - system prompt failures are non-blocking warnings
- **Cleaner code** - uses existing template infrastructure more efficiently
- **Same functionality** - system prompt integration still works, just simpler

### Code changes made:
- `actions.rs:6`: Changed import from `claude_code_integration` to `system_prompt::render_system_prompt`
- `actions.rs:405-466`: Replaced complex integration logic with direct approach
- All tests pass ✅
- No clippy warnings ✅

The issue is now **resolved**. The `claude_code_integration.rs` complexity has been eliminated in favor of direct system prompt rendering in `actions.rs` where we already have all the necessary infrastructure.