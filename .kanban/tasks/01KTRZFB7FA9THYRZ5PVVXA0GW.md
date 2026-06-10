---
assignees:
- claude-code
position_column: todo
position_ordinal: e480
title: filter_integration::s17_tag_names_with_special_chars fails at HEAD — merge reverted tag-parser slug charset tightening
---
## What

`cargo nextest run -p swissarmyhammer-kanban` fails deterministically (3/3 runs) at committed HEAD:

```
FAIL swissarmyhammer-kanban::filter_integration s17_tag_names_with_special_chars
assertion `left == right` failed at crates/swissarmyhammer-kanban/tests/filter_integration.rs:519
  left: Number(0)   right: 1
```

`add task` with description `#v2.0 release` then `list tasks` filter `#v2` returns 0 tasks.

## Diagnosis

Commit eee2ba9b3 (`fix(kanban): keep normalize_slug in sync with tightened tag charset`) updated this test to expect the tightened `[A-Za-z0-9-]` slug charset: `#v2.0` trims at the dot → tag `v2`. But the current `crates/swissarmyhammer-kanban/src/tag_parser.rs` at HEAD is self-contradictory after the later merge `606685949` (origin/main into plugin):

- module docs / slug-run comment say the run is `[A-Za-z0-9_-.]` (dots and underscores included, `#v2.0` keeps the dot)
- `parse_tags`'s other inline comment and `normalize_slug`'s docs/tests say `[A-Za-z0-9-]`

So the parser produces tag `v2.0`, the filter `#v2` matches nothing, and the test (written for the tightened charset) fails. The merge appears to have brought back main's looser parser body while keeping the plugin branch's tightened test + `normalize_slug`.

## Fix

Re-apply the tightened `[A-Za-z0-9-]` slug-run in `parse_tags` (the b8964d9f9 / eee2ba9b3 contract), reconcile the module docs (they currently disagree with themselves), and confirm `normalize_slug` round-trip tests still hold. Verify with `cargo nextest run -p swissarmyhammer-kanban`.

Discovered while implementing 01KTRMXRNH66GZCWSNR1YGE28E (caption rendering) — unrelated to that change (verified: failure reproduces with only caption-casing edits in the tree, which do not touch tag parsing).