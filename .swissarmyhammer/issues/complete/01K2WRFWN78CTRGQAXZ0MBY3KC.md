the info messages in swissarmyhammer_tools::mcp::tools::web_search::duckduckgo_client need to be debug

## Proposed Solution

The issue requires changing `info` level log messages to `debug` level in the DuckDuckGo client. After analyzing the code, I found the following `tracing::info!` statements that should be changed to `tracing::debug!`:

1. Line 271-274: "Retrieved HTML content of {} characters from DuckDuckGo page"
2. Line 297: "Starting to parse HTML results from DuckDuckGo page"
3. Line 301: "DuckDuckGo search found {} results"
4. Line 333-336: "Parsing HTML content of {} characters for search results"
5. Line 380: "No results found with selector: {}"
6. Line 384-388: "Found {} potential results with selector: {}"
7. Line 441-444: "Skipping invalid result: title='{}', url='{}'"
8. Line 453: "Invalid result element HTML: {}"

These are detailed operational messages that are more appropriate for debug-level logging rather than info-level. The change will reduce log noise at the info level while preserving detailed debugging information when needed.

## Implementation Steps

1. Change all `tracing::info!` calls to `tracing::debug!` in the identified lines
2. Verify the changes with tests
3. Ensure all functionality remains intact

## Implementation Complete

Successfully changed all `tracing::info!` log messages to `tracing::debug!` in the DuckDuckGo client. The following 8 logging statements were converted:

1. ✅ Line 271-274: "Retrieved HTML content of {} characters from DuckDuckGo page"
2. ✅ Line 297: "Starting to parse HTML results from DuckDuckGo page"
3. ✅ Line 301: "DuckDuckGo search found {} results"
4. ✅ Line 333-336: "Parsing HTML content of {} characters for search results"
5. ✅ Line 380: "No results found with selector: {}"
6. ✅ Line 384-388: "Found {} potential results with selector: {}"
7. ✅ Line 441-444: "Skipping invalid result: title='{}', url='{}'"
8. ✅ Line 453: "Invalid result element HTML: {}"

## Validation Results

- ✅ All tests pass (329/329)
- ✅ Code formatting clean (`cargo fmt`)
- ✅ No clippy warnings (`cargo clippy`)
- ✅ DuckDuckGo client specific tests all pass (12/12)

The changes reduce log noise at the info level while preserving detailed debugging information when needed. All functionality remains intact and the implementation follows Rust logging best practices.