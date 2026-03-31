---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
title: '[WARNING] YAML JSONL timestamp resolution is a no-op — ours always wins'
---
`swissarmyhammer-merge/src/yaml.rs:170-239`\n\nWhen a JSONL path is provided, the code reads the changelog and builds two timestamp maps (`ours_timestamps` and `theirs_timestamps`), but then immediately overwrites `theirs_timestamps` with `ours_timestamps.clone()` (line ~188). Both maps end up identical because there is only one JSONL file being read. The conflict resolution predicate at line 239 (`if ot >= tt`) therefore always evaluates to true (equal timestamps), meaning ours unconditionally wins regardless of actual change history. The \"newest-wins from JSONL\" feature does not work as intended.\n\nTo fix this correctly, two separate JSONL files are needed — one for ours and one for theirs — so the timestamps can actually differ. Alternatively, remove the JSONL-based resolution path entirely if the feature is not ready, rather than silently siloing a broken fast-path. The test at line 397 acknowledges the issue with a loose assertion and should be tightened once the logic is correct." #review-finding