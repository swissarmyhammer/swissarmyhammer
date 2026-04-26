---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffc280
title: 'WARNING: glob::Pattern::new called per-file per-pattern in hot path with no caching'
---
avp-common/src/validator/runner.rs (filter_changed_files_for_ruleset, filter_diffs_for_ruleset) and avp-common/src/validator/types.rs (matches_files, matches_changed_files)\n\n`glob::Pattern::new(pattern)` is called inside a nested loop: for each file, for each pattern. Pattern compilation involves parsing and is not free. With many changed files (e.g., a large refactor touching 100+ files) and multiple patterns per ruleset, this becomes O(files * patterns * parse_cost).\n\nThe pattern set is fixed per-ruleset, so patterns should be compiled once and reused.\n\nSuggestion: Compile patterns once before the loop:\n```rust\nlet compiled: Vec<glob::Pattern> = patterns.iter()\n    .filter_map(|p| glob::Pattern::new(p).ok())\n    .collect();\n```\nThen match against the compiled list. This applies to all four call sites." #review-finding