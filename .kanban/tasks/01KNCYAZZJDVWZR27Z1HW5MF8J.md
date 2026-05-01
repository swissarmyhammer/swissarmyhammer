---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffc180
title: 'WARNING: filter_changed_files_for_ruleset and filter_diffs_for_ruleset duplicate glob matching logic'
---
avp-common/src/validator/runner.rs:708-770\n\nThese two functions are structurally identical -- they differ only in the type being filtered (String vs FileDiff) and how the path is extracted. Both build MatchOptions, iterate patterns, and call glob::Pattern::new in the same nested loop.\n\nThis is also the third copy of the same glob-matching logic (the other two are in types.rs: Validator::matches_files and Validator::matches_changed_files).\n\nSuggestion: Extract a shared helper like `fn matches_any_pattern(path: &str, patterns: &[String]) -> bool` and use it in all four locations. This would also be the natural place to add pattern pre-compilation (see the caching finding)." #review-finding