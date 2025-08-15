# Add Parameter Validation and Configuration

## Overview
Implement comprehensive parameter validation and configurable options for the web_fetch tool, including timeout, redirects, content limits, and custom headers. Refer to /Users/wballard/github/sah-fetch/ideas/fetch.md.

## Tasks
- Extend JSON schema to include all optional parameters (timeout, follow_redirects, max_content_length, user_agent)
- Implement parameter validation with appropriate defaults
- Add URL validation and sanitization
- Configure HTTP client with user-provided parameters
- Add parameter bounds checking and validation errors

## Implementation Details
- Update JSON schema with optional parameters and their constraints
- Set parameter defaults: timeout (30s), follow_redirects (true), max_content_length (1MB)
- Validate URL format and scheme (HTTP/HTTPS only)
- Configure markdowndown client with user parameters
- Add comprehensive parameter validation with clear error messages

## Success Criteria
- All specification parameters are supported and validated
- Default values match specification requirements
- Invalid parameters generate clear error messages
- URL validation prevents malformed URLs
- Parameter bounds are enforced correctly

## Dependencies
- Requires fetch_000004_html-markdown-conversion (for core functionality)

## Estimated Impact
- Provides full parameter flexibility as per specification
- Adds safety through validation