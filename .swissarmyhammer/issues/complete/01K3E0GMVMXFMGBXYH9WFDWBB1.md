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

## Implementation Progress

✅ **Code Review Task Completed**: Successfully addressed the remaining todo item from the code review process:

### Changes Made
1. **Extracted Common Function**: Created `common::prompt_utils::render_system_prompt()` function to eliminate code duplication between:
   - `swissarmyhammer/src/workflow/actions.rs` 
   - `tests/system_prompt_integration_tests.rs`

2. **Updated Module Exports**: Added the new function to `common/mod.rs` exports

3. **Updated Function Calls**: Replaced all `render_system_prompt_via_library()` calls with the new shared `render_system_prompt()` function

4. **Verified Compilation**: All packages build successfully with no clippy warnings

### Code Quality Improvements
- **Eliminated Duplication**: No more identical logic in multiple files
- **Centralized Implementation**: System prompt rendering logic now lives in one place
- **Consistent API**: Both workflow actions and tests use the same implementation
- **Maintainable**: Future changes only need to be made in one location

### Files Modified
- `swissarmyhammer/src/common/prompt_utils.rs` (created)
- `swissarmyhammer/src/common/mod.rs` (updated exports)  
- `swissarmyhammer/src/workflow/actions.rs` (removed duplicate function, updated calls)
- `tests/system_prompt_integration_tests.rs` (removed duplicate function, updated calls)

The refactoring maintains all existing functionality while eliminating code duplication and improving maintainability. The system prompt continues to be rendered using the standard PromptLibrary infrastructure as intended.