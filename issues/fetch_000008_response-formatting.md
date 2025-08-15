# Implement Response Formatting and Metadata

## Overview
Implement comprehensive response formatting with detailed metadata as specified in the web_fetch tool specification. Refer to /Users/wballard/github/sah-fetch/ideas/fetch.md.

## Tasks
- Structure response format to match specification exactly
- Include all required metadata fields (url, final_url, title, content_type, etc.)
- Add optional metadata (headers, word_count, response_time_ms)
- Implement success, redirect, and error response formats
- Add content statistics and timing information

## Implementation Details
- Format successful responses with content and metadata sections
- Include timing information (response_time_ms) for performance monitoring
- Add word count calculation for markdown content
- Extract and include relevant HTTP headers in response
- Structure error responses with detailed error information
- Ensure response format matches specification examples

## Success Criteria
- Response format exactly matches specification examples
- All required metadata fields are included
- Timing and statistics are accurate
- Error responses provide useful debugging information
- Response is properly structured for MCP protocol

## Dependencies
- Requires fetch_000007_error-handling-security (for complete error handling)

## Estimated Impact
- Provides rich information for tool users
- Enables performance monitoring and debugging