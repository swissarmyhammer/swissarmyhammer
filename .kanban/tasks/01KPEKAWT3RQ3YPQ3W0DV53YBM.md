---
assignees:
- claude-code
position_column: todo
position_ordinal: da80
title: Decide fate of 5 filter-excluded embedding tests in default profile
---
What: `.config/nextest.toml` sets `default-filter = "not (package(swissarmyhammer-embedding) & test(/(qwen_embedding|unixcoder_)/))"`, which silently excludes 5 tests from every default `cargo nextest run --workspace`. They are only run via `--profile embedding-models`.

This is a skipped-test pattern dressed up as a filter. Per project test discipline, skipped tests are not acceptable in the default run.

Options:
1. Mark these tests `#[cfg(feature = "embedding-models")]` and only compile them when the feature is enabled, so the default `cargo nextest run` compiles and runs a strict subset with no filter-based exclusions.
2. Move them to `tests/local/` with an opt-in path (same effect, less machinery).
3. Delete them if coverage is redundant.

Acceptance Criteria:
- `cargo nextest run --workspace` in a fresh checkout reports `0 skipped` (no `default-filter` exclusions).
- Whatever opt-in mechanism replaces the filter is documented in the crate README.

Tests: post-change, verify `cargo nextest run --workspace 2>&1 | grep -c "skipped"` equals 0 (modulo `#[ignore]` which is tracked separately). #test-failure