# Add Integration Tests with Real Web Requests

## Overview
Implement integration tests that make real HTTP requests to test the web_fetch tool end-to-end functionality including HTML-to-markdown conversion and response handling. Refer to /Users/wballard/github/sah-fetch/ideas/fetch.md.

## Tasks
- Create integration tests with real web requests to stable test endpoints
- Test HTML-to-markdown conversion with actual web content
- Test redirect handling with real redirect chains
- Test timeout and error handling with controlled scenarios
- Add tests for different content types and character encodings

## Implementation Details
- Use stable, reliable test endpoints (httpbin.org, example.com, etc.)
- Test successful fetching and markdown conversion
- Test redirect scenarios with known redirect URLs
- Test timeout scenarios with controlled delays
- Test different character encodings and content types
- Use controlled test data to ensure predictable results
- Follow existing integration test patterns in codebase

## Success Criteria
- Integration tests pass consistently
- Real web content is fetched and converted correctly
- Redirect handling works with actual redirects
- Error conditions are handled properly
- Tests are reliable and not flaky
- Integration with MCP context works correctly

## Dependencies
- Requires fetch_000009_unit-tests (for complete test coverage)

## Estimated Impact
- Validates tool works with real web content
- Ensures end-to-end functionality is correct