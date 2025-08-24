the whole system_prompt.rs module should not exist, the system prompt should jsut be rendered like any other prompt... it is just called .system
the whole system_prompt.rs module should not exist, the system prompt should jsut be rendered like any other prompt... it is just called .system

## Analysis

The current system has a dedicated `system_prompt.rs` module with:
- Complex caching logic that was recently disabled 
- Special file search paths and custom rendering logic
- SystemPromptRenderer with partial support
- Used in workflow actions and tests

The `.system.md` file is located at `/builtin/prompts/.system.md` and should be treated like any other prompt.

## Proposed Solution

1. **Remove system_prompt.rs module entirely**
2. **Update all references to use standard PromptLibrary**:
   - Replace `render_system_prompt()` calls with `library.get(".system")?.render(args)`
   - Update workflow actions to use normal prompt rendering
   - Update tests to use PromptLibrary instead of SystemPromptRenderer
3. **Update lib.rs exports** to remove system_prompt module exports
4. **Verify .system.md is accessible** through normal prompt discovery

The `.system` prompt should work exactly like any other prompt - loadable via PromptLibrary and renderable with Template::render_with_config().

## Benefits

- Eliminates special-case code and complexity
- Uses consistent prompt rendering pipeline 
- Removes caching complexity that was already disabled
- Follows the principle that system prompt is "just another prompt"