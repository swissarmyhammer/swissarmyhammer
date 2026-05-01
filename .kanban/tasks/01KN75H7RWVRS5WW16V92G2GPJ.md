---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffc480
title: Test web fetch error paths and categorize_error branches
---
File: swissarmyhammer-web/src/fetch.rs (62.4%, 32 uncovered lines)\n\nUncovered functions/paths:\n- `fetch_url()` error path (lines 105-127): the `markdowndown::convert_url_with_config` failure branch\n- `categorize_error()` (lines 134-173): many error category branches untested (ssl_error, redirect_error, auth_error, not_found, client_error, server_error, content_error, size_limit_error)\n\nTests needed:\n- Unit tests for categorize_error with various error message strings\n- Integration test for fetch_url failure path (mock or controlled URL)\n\nAcceptance: coverage >= 80% for fetch.rs #coverage-gap