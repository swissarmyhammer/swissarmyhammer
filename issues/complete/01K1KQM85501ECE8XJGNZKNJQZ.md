Log actions need to render with liquid, using the current workflow variable context and all available variables.

This is in example-actions.md - what happends is -- {{branch_value}} actually gets printed, which exactly nobody is going to want.

- Branch1: Log "Branch 1 selected: {{branch_value}} contains Cargo"


## Proposed Solution

After analyzing the codebase, I found the root cause of the issue:

1. **Current Implementation**: The `LogAction::execute` method on line 797 in `/swissarmyhammer/src/workflow/actions.rs` calls `self.substitute_string(&self.message, context)` which only handles `${variable}` syntax through the `substitute_variables_safe` method.

2. **Liquid Template Engine**: The codebase already has full liquid template support and uses it in `parse_action_from_description_with_context` for action parsing, but this is not used for runtime log message rendering.

3. **The Fix**: Modify the `LogAction::execute` method to use liquid template rendering instead of the basic variable substitution, making it consistent with the workflow's liquid template capabilities.

### Implementation Plan

1. **Update LogAction::execute method** to use liquid template rendering for message processing
2. **Convert the workflow context** to liquid variables format  
3. **Render the log message** using the liquid template engine with `{{variable}}` syntax support
4. **Add fallback** to the current variable substitution for backward compatibility
5. **Test with the example-actions.md** workflow to verify `{{branch_value}}` renders correctly

This approach will:
- Enable `{{branch_value}}` syntax in log messages 
- Maintain backward compatibility with `${variable}` syntax
- Use the existing liquid template infrastructure
- Follow the established pattern of liquid template usage in the codebase

The fix will be in the `LogAction::execute` method where instead of:
```rust
let message = self.substitute_string(&self.message, context);
```

We'll use liquid template rendering:
```rust 
let message = render_with_liquid_template(&self.message, context)
    .unwrap_or_else(|| self.substitute_string(&self.message, context));
```