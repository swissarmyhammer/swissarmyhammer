---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffef80
title: Test WebSearcher config loading methods
---
File: swissarmyhammer-web/src/search/mod.rs (37.5%, 60 uncovered lines)\n\nUncovered functions:\n- `load_content_fetch_config()` (lines 47-137): all config-loading branches for content fetching params (max_concurrent, timeout, max_content_size, domain_delay, min/max content length, summary length, code blocks, summaries, metadata)\n- `load_scoring_config()` (lines 141-181): scoring config loading branches (base_score, position_penalty, min_score, exponential_decay, decay_rate)\n- `parse_size_string()` (lines 184-195): MB/KB/GB/raw parsing\n- `validate_request()` (lines 198+): language/query validation\n\nTests needed:\n- Unit test for parse_size_string with MB, KB, GB, raw values\n- Unit test for validate_request with empty query, long query, bad language\n- Config loading tests with mock template context\n\nAcceptance: coverage >= 80% for search/mod.rs #coverage-gap