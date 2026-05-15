---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffe580
title: Test Brave search client error paths
---
File: swissarmyhammer-web/src/search/brave.rs (71.1%, 22 uncovered lines)\n\nUncovered paths:\n- Lines 64-96: HTTP error responses from Brave API (non-success status, response parsing failures, empty results)\n- Line 174: error path in search result scoring\n- Line 222: edge case in result processing\n\nTests needed:\n- Mock HTTP responses for Brave API errors\n- Test empty/malformed search results handling\n\nAcceptance: coverage >= 80% for brave.rs #coverage-gap