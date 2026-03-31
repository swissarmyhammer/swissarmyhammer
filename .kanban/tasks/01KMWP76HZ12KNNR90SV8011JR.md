---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffff9480
title: Add tests for resolve_conflict (yaml.rs)
---
swissarmyhammer-merge/src/yaml.rs:288-334\n\n`fn resolve_conflict(key, ours, theirs, value_timestamps, fallback_precedence) -> &Value`\n\nResolves a single field conflict using changelog matching or fallback. Tested indirectly via merge_yaml tests, but the function itself is private and exercises several branches:\n- Ours matches logged value, theirs doesn't → ours wins\n- Theirs matches logged value, ours doesn't → theirs wins\n- Neither matches → fallback\n- No entry in changelog for this key → fallback\n- Both match (shouldn't happen but the code handles it) → fallback\n\nThese branches ARE covered by the three JSONL conflict tests, but only at one level of indirection. Consider whether direct unit tests add value or if the existing integration-style tests suffice. #coverage-gap