do not cache the system prompt

## Proposed Solution

After analyzing the system prompt implementation in `swissarmyhammer/src/system_prompt.rs`, I found that there is a comprehensive caching system in place:

1. **Global Cache**: `SYSTEM_PROMPT_CACHE` - a static global mutex holding cached content
2. **Cache Validation**: Methods to check if cached content is still valid based on file modification times
3. **Cache Management**: Methods to store and retrieve cached content

To implement "do not cache the system prompt", I will:

1. Remove the caching logic from the `SystemPromptRenderer::render()` method
2. Always re-read and re-render the system prompt file on every call
3. Remove the cache validation methods since they won't be needed
4. Keep the `clear_cache()` function as a no-op for backward compatibility
5. Update tests to reflect the non-caching behavior

The key changes will be in the `render()` method - instead of checking and using the cache, it will always:
- Find the system prompt file
- Read the content from disk
- Render the template
- Return the result without caching

This ensures that any changes to the system prompt file or its partials are immediately reflected without requiring cache invalidation.

## Implementation Complete

Successfully implemented the removal of system prompt caching. Here's what was changed:

### Changes Made

1. **Removed Cache Checking Logic**: Modified `SystemPromptRenderer::render()` to always re-read and re-render the system prompt file without checking any cache
2. **Removed Cache Storage**: Eliminated the logic that stored rendered content in the global cache
3. **Updated `clear_cache()` Function**: Converted it to a no-op function for backward compatibility
4. **Cleaned Up Unused Code**: Removed all cache-related data structures and methods:
   - `CacheEntry` struct 
   - `SYSTEM_PROMPT_CACHE` global variable
   - `get_partial_mtimes()` method
   - `is_cache_valid()` method  
   - `extract_partial_names()` method
   - `get_mtime()` method
5. **Updated Documentation**: Modified module and function documentation to reflect non-caching behavior
6. **Cleaned Up Tests**: Removed or updated tests that were specific to caching functionality
7. **Updated Imports**: Removed unused imports (`SystemTime`, `Mutex`)

### Key Benefits

- **Always Fresh Content**: System prompt and its partials are always read fresh from disk
- **Immediate Reflection of Changes**: Any modifications to `.system.md` or its partial files are immediately visible
- **Simplified Code**: Removed complex cache validation and management logic
- **Backward Compatibility**: Existing code that calls `clear_cache()` continues to work

### Testing

- ✅ All unit tests pass 
- ✅ All integration tests pass
- ✅ Cargo clippy passes with no warnings
- ✅ Clean build succeeds

The system prompt will now always be rendered fresh on each call to ensure up-to-date content.