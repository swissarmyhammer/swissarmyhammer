PromptAction must NOT shell out to swissarmyhammer, it needs to just called the code to render the prompt string. This can still be piped to claude

## Proposed Solution

I've analyzed the code and found that the issue is in the `PromptAction::render_prompt_with_swissarmyhammer` method at `swissarmyhammer/src/workflow/actions.rs:356-407`. Currently, it shells out to the swissarmyhammer binary:

```rust
// Build the command to render the prompt
let cmd_binary = resolve_swissarmyhammer_binary();
let mut cmd = Command::new(&cmd_binary);
cmd.arg("prompt")
    .arg("test")
    .arg(&self.prompt_name)
    .arg("--raw");
```

**The solution is to replace this shell execution with direct calls to the prompt rendering logic:**

1. **Replace the shell execution** with direct calls to:
   - `PromptResolver::new()` and `PromptResolver::load_all_prompts()` to load prompts
   - `PromptLibrary::render_prompt_with_env(prompt_name, &args)` to render the prompt

2. **Import necessary dependencies** in the actions.rs file:
   - `use swissarmyhammer::{PromptResolver, PromptLibrary};`

3. **The new flow will be:**
   ```rust
   async fn render_prompt_directly(&self, args: &HashMap<String, String>) -> ActionResult<String> {
       let mut library = PromptLibrary::new();
       let mut resolver = PromptResolver::new();
       resolver.load_all_prompts(&mut library)?;
       let rendered = library.render_prompt_with_env(&self.prompt_name, args)?;
       Ok(rendered)
   }
   ```

This eliminates the subprocess overhead and makes the workflow execution more reliable and faster, while still producing the exact same result that can be piped to Claude.
## Implementation Complete

The issue has been successfully resolved. Here's what was implemented:

### Changes Made

1. **Updated `swissarmyhammer/src/workflow/actions.rs`:**
   - Added imports: `use crate::{PromptLibrary, PromptResolver};`
   - Replaced `render_prompt_with_swissarmyhammer()` method with direct calls to prompt rendering logic
   - Removed unused `resolve_swissarmyhammer_binary()` function
   - Renamed method to `render_prompt_directly()` for clarity

2. **New Implementation:**
   ```rust
   async fn render_prompt_directly(&self, context: &HashMap<String, Value>) -> ActionResult<String> {
       // Substitute variables in arguments
       let args = self.substitute_variables(context);

       // Validate argument keys (same validation as before)
       for key in args.keys() {
           if !is_valid_argument_key(key) {
               return Err(ActionError::ParseError(format!("Invalid argument key '{key}'")));
           }
       }

       // Load prompts and render directly (instead of shelling out)
       let mut library = PromptLibrary::new();
       let mut resolver = PromptResolver::new();
       
       resolver.load_all_prompts(&mut library).map_err(|e| {
           ActionError::ClaudeError(format!("Failed to load prompts: {e}"))
       })?;

       let rendered = library.render_prompt_with_env(&self.prompt_name, &args).map_err(|e| {
           ActionError::ClaudeError(format!("Failed to render prompt '{}': {}", self.prompt_name, e))
       })?;

       Ok(rendered)
   }
   ```

### Benefits Achieved

- ✅ **No more subprocess overhead**: Eliminates the need to spawn a separate `swissarmyhammer` process
- ✅ **Faster execution**: Direct function calls are significantly faster than subprocess execution
- ✅ **Same functionality**: Produces identical output - the rendered prompt string can still be piped to Claude
- ✅ **Better error handling**: More precise error messages without subprocess complexity
- ✅ **Cleaner code**: Removed complex binary resolution logic that's no longer needed

### Verification

- Code compiles successfully with `cargo check`
- All existing validation and error handling preserved
- Same API surface - no breaking changes to consumers

The PromptAction now calls the prompt rendering code directly instead of shelling out, achieving the goal stated in the issue.