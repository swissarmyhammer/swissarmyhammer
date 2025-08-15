# Implement Redirect Handling and Tracking

## Overview
Add comprehensive HTTP redirect handling with redirect chain tracking and limits as specified in the web_fetch tool specification. Refer to /Users/wballard/github/sah-fetch/ideas/fetch.md.

## Tasks
- Configure markdowndown client for redirect handling
- Track redirect chains and count redirects
- Set maximum redirect limit (10 as per specification)
- Return redirect information in response metadata
- Handle different redirect status codes (301, 302, 303, 307, 308)

## Implementation Details
- Configure client max_redirects based on follow_redirects parameter
- Capture and track redirect chain with status codes
- Include redirect_count and redirect_chain in response metadata
- Return final_url in response to show redirect destination
- Handle redirect loops and excessive redirects

## Success Criteria
- Redirects are followed correctly when enabled
- Redirect chains are tracked and reported
- Maximum redirect limit prevents infinite loops
- Response includes redirect metadata as per specification
- Different redirect types are handled appropriately

## Dependencies
- Requires fetch_000005_parameter-validation (for parameter support)

## Estimated Impact
- Enables handling of URLs with redirects
- Provides transparency about redirect behavior