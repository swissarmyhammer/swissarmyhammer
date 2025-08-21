Clean up any dead code in the web_search tool.

## Analysis

After examining the web_search module structure, I've identified one major piece of dead code:

### Dead Code Found

1. **`duckduckgo_api_client.rs`** - Completely unused module
   - Contains `DuckDuckGoApiClient`, `DuckDuckGoApiConfig`, `DuckDuckGoApiError`, and `DuckDuckGoApiResponse`
   - Module is declared in `mod.rs` but never imported or used anywhere in the codebase
   - The web search tool only uses `DuckDuckGoClient` from `duckduckgo_client.rs` 
   - This appears to be a legacy implementation that was replaced by the current HTML scraping approach

### Modules Currently Used

- `content_fetcher.rs` - Used by search/mod.rs 
- `duckduckgo_client.rs` - Used by search/mod.rs (main implementation)
- `privacy.rs` - Used by multiple modules
- `search/mod.rs` - Contains the main WebSearchTool
- `types.rs` - Used by multiple modules

## Proposed Solution

1. Remove the unused `duckduckgo_api_client.rs` file
2. Remove the `pub mod duckduckgo_api_client;` declaration from `mod.rs`
3. Run tests to ensure nothing breaks
4. Run clippy and cargo check to verify no warnings

This cleanup will:
- Reduce codebase size by ~616 lines
- Remove maintenance burden of unused code
- Simplify the module structure
- Remove potential confusion about which client is actually used

## Implementation Completed

Successfully cleaned up dead code in the web_search tool:

### Changes Made

1. **Removed `duckduckgo_api_client.rs`** (616 lines)
   - Deleted the entire unused module
   - Removed module declaration from `mod.rs`

### Verification Results

✅ **Compilation**: `cargo check --all-features` - Success  
✅ **Tests**: All 49 web_search tests pass  
✅ **Linting**: `cargo clippy --all-features` - No warnings  

### Impact

- **Code reduction**: Removed 616 lines of unused code
- **Simplified structure**: Cleaner module organization 
- **Maintained functionality**: No breaking changes to existing web search features
- **Reduced maintenance burden**: One less module to maintain

The web_search tool now only contains actively used code:
- `content_fetcher.rs` - Content fetching functionality
- `duckduckgo_client.rs` - Main HTML scraping implementation  
- `privacy.rs` - Privacy and anonymization features
- `search/mod.rs` - Primary WebSearchTool implementation
- `types.rs` - Type definitions

**Status**: ✅ Complete - Dead code successfully removed without breaking functionality.