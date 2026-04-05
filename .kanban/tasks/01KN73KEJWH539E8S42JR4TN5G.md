---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffc980
title: Add tests for ContentFetcher processing methods
---
src/search/content_fetcher.rs:557-655, 688-822\n\nCoverage: 18.7% (59/316 lines)\n\nUncovered lines: 557-610, 613-655, 688-822\n\n```rust\nfn extract_key_points(&self, content: &str) -> Vec<String>\nfn extract_code_blocks(&self, content: &str) -> Vec<CodeBlock>\nfn extract_metadata(&self, content: &str, result: &SearchResult) -> ContentMetadata\nfn classify_content_type(&self, url: &str, content: &str) -> ContentType\nfn extract_title_from_content(&self, content: &str) -> Option<String>\nfn detect_language(&self, content: &str) -> Option<String>\nfn extract_tags(&self, content: &str) -> Vec<String>\n```\n\nAll content processing methods are untested. Each is pure and easily testable:\n1. extract_key_points — bullet lists, numbered lists, indicator words\n2. extract_code_blocks — fenced blocks with/without language, inline code fallback\n3. classify_content_type — URL patterns (/docs/, /blog/, /tutorial/), content patterns\n4. extract_title_from_content — markdown heading extraction\n5. detect_language — English/Spanish/French keyword heuristics\n6. extract_tags — tech keyword and hashtag extraction #coverage-gap