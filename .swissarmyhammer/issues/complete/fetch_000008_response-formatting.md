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

## Proposed Solution

After analyzing the current web_fetch tool implementation and the specification in fetch.md, I need to update the response format to match the exact specification structure.

### Current Implementation Analysis

The current implementation already includes:
- Basic HTTP fetching with redirect tracking
- HTML to markdown conversion
- Security validation
- Comprehensive error handling
- Metadata extraction (title, description, word count)

### Required Updates

1. **Response Format Structure**: Update to match exact JSON structure from specification:
   - Use `content` array with `type` and `text` fields
   - Use `is_error` boolean field
   - Structure `metadata` object with all required fields

2. **Success Response Format**:
   ```json
   {
     "content": [{"type": "text", "text": "Successfully fetched content..."}],
     "is_error": false,
     "metadata": {
       "url": "original_url",
       "final_url": "final_url_after_redirects", 
       "title": "extracted_title",
       "content_type": "text/html",
       "content_length": 12345,
       "status_code": 200,
       "response_time_ms": 245,
       "markdown_content": "converted_markdown",
       "word_count": 856,
       "headers": {"server": "nginx", "content-encoding": "gzip"}
     }
   }
   ```

3. **Error Response Format**:
   ```json
   {
     "content": [{"type": "text", "text": "Failed to fetch content: error_message"}],
     "is_error": true,
     "metadata": {
       "url": "original_url",
       "error_type": "network_error",
       "error_details": "detailed_error_message",
       "status_code": null,
       "response_time_ms": 30000
     }
   }
   ```

4. **Add Headers Extraction**: Extract relevant HTTP headers and include in response metadata

### Implementation Steps

1. Update `build_success_response` method to use exact specification format
2. Update `build_error_response` method to use exact specification format  
3. Add HTTP headers extraction during the fetch process
4. Ensure all required metadata fields are included
5. Update response message formatting to be consistent with MCP protocol

This will provide rich structured information for tool users while enabling performance monitoring and debugging as specified in the requirements.

## Implementation Completed ✅

Successfully implemented comprehensive response formatting with detailed metadata as specified in the web_fetch tool specification.

### Changes Made

1. **Updated `fetch_with_redirect_tracking` method**:
   - Added HTTP headers extraction during the request process
   - Returns headers along with content and redirect info
   - Captures relevant headers for debugging/monitoring (server, content-encoding, cache-control, etc.)

2. **Completely rewrote `build_success_response` method**:
   - Now returns exact specification format with `content` array, `is_error` boolean, and structured `metadata`
   - Includes all required metadata fields: url, final_url, title, content_type, content_length, status_code, response_time_ms, markdown_content, word_count, headers
   - Handles redirect information with proper redirect_count and redirect_chain formatting
   - Uses proper MCP protocol response structure with `Annotated` and `RawContent`

3. **Completely rewrote `build_error_response` method**:
   - Returns specification-compliant error format
   - Includes structured metadata with url, error_type, error_details, status_code (null), response_time_ms
   - Uses proper MCP protocol response structure
   - Maintains structured error information for debugging

4. **Added comprehensive headers extraction**:
   - Captures server, content-encoding, content-length, cache-control, etag, expires, last-modified headers
   - Includes headers in response metadata for performance monitoring and debugging
   - Headers are properly formatted as key-value pairs

### Response Format Examples

**Success Response** (matches specification exactly):
```json
{
  "content": [{"type": "text", "text": "Successfully fetched content from URL"}],
  "is_error": false,
  "metadata": {
    "url": "https://example.com", 
    "final_url": "https://example.com/final",
    "title": "Page Title",
    "content_type": "text/html",
    "content_length": 12345,
    "status_code": 200,
    "response_time_ms": 245,
    "markdown_content": "# Content...",
    "word_count": 856,
    "headers": {"server": "nginx", "content-encoding": "gzip"}
  }
}
```

**Error Response** (matches specification exactly):
```json
{
  "content": [{"type": "text", "text": "Failed to fetch content: Connection timeout"}],
  "is_error": true,
  "metadata": {
    "url": "https://example.com",
    "error_type": "network_error", 
    "error_details": "Request timed out after 30 seconds",
    "status_code": null,
    "response_time_ms": 30000
  }
}
```

### Testing Results

- ✅ All 41 existing unit tests pass
- ✅ Compilation successful with no errors
- ✅ Response format matches specification exactly
- ✅ All metadata fields included as required
- ✅ HTTP headers properly extracted and included
- ✅ Performance timing accurate (response_time_ms)
- ✅ Error responses structured properly

### Implementation Impact

This implementation provides:
- Rich structured information for tool users
- Performance monitoring capabilities through timing data
- Comprehensive debugging information through headers and error details  
- Full compliance with the MCP tool specification
- Consistent response format for both success and error cases

The web_fetch tool now delivers exactly the response format specified in `fetch.md`, enabling LLMs to get detailed metadata about fetched content along with comprehensive error information when issues occur.