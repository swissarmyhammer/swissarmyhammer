---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffde80
title: Add tests for WebSearcher config loading and parse_size_string
---
src/search/mod.rs:27-195\n\nCoverage: 14.9% (15/101 lines)\n\nUncovered lines: 27-56, 61-134, 141-195\n\n```rust\npub fn get_search_client(&mut self) -> &BraveSearchClient\nfn load_config_with_callback<T, F>(...) -> T\npub fn load_content_fetch_config() -> ContentFetchConfig\nfn load_scoring_config() -> ScoringConfig\nfn parse_size_string(size_str: &str) -> Result<usize, ...>\n```\n\nMajor untested functions:\n1. get_search_client() — lazy init of BraveSearchClient\n2. load_content_fetch_config() — all config key extraction branches (lines 47-137)\n3. load_scoring_config() — scoring config extraction (lines 141-181)\n4. parse_size_string() — parse \"2MB\", \"512KB\", \"1GB\", plain number (lines 184-195)\n\nparse_size_string is pure and easily testable. Config loading depends on swissarmyhammer_config but can test the callback pattern. #coverage-gap