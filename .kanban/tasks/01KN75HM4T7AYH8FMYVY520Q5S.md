---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffe780
title: 'Test ContentFetcher async pipeline: fetch_search_results, fetch_single_result, rate limiting'
---
File: swissarmyhammer-web/src/search/content_fetcher.rs (59.9%, 123 uncovered lines)\n\nUncovered functions/paths:\n- `fetch_search_results()` (lines 256-311): concurrent fetching pipeline with error stat tracking (rate_limited, quality_filtered, failed, task join errors)\n- `fetch_single_result()` (lines 315-400): HTTP error handling (timeout, connect, non-success status), HTML truncation, quality check, content processing (summary, key_points, code_blocks, metadata)\n- `wait_for_domain()` (lines 436-462): rate limiting with max_domain_delay exceeded case\n- `generate_summary()` (lines 522-554): extractive summarization\n- `extract_code_blocks()` (lines 613-655): fenced and inline code block extraction\n- `extract_metadata()` (lines 658-674): metadata extraction with reading time\n\nTests needed:\n- Async tests with mock HTTP server for fetch_search_results error paths\n- Unit tests for generate_summary, extract_code_blocks, extract_metadata\n- Rate limiting test for wait_for_domain max delay exceeded\n\nAcceptance: coverage >= 70% for content_fetcher.rs #coverage-gap