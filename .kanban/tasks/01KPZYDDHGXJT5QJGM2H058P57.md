---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe880
project: skills-guide-review
title: Add user trigger phrases to `really-done` description
---
## What

Current `builtin/skills/really-done/SKILL.md` description:

> Use when about to claim work is complete, fixed, or passing, before committing or creating PRs - requires running verification commands and confirming output before making any success claims; evidence before assertions always

The description describes the WHAT and has a broad WHEN, but lacks specific user-facing phrases. Users say things like "are we done?", "is it working?", "ready to ship?", "really done?" which should appear in the trigger list.

## Acceptance Criteria

- [x] Description adds specific user trigger phrases ("really done", "are we done", "ready to ship", "ready to commit", "is this passing").
- [x] Tightens the run-on sentence structure.
- [x] Under 1024 chars, no `<`/`>`.

## Tests

- [x] Trigger test: "are we done here?" → loads `really-done`.
- [x] Trigger test: a pure coding question unrelated to completion should NOT load it.

## Reference

Anthropic guide, Chapter 2 — "The description field".

## Implementation Notes

New description (378 chars, no `<`/`>`):

> Verify work before claiming it done. Use when the user says "really done", "are we done", "ready to ship", "ready to commit", "is this passing", or when about to claim work is complete, fixed, or passing. Also use before committing or creating PRs. Requires running verification commands and confirming output before any success claim — evidence before assertions, always.

Trigger design:
- "are we done here?" phrase-matches "are we done" — loads.
- A pure coding question (e.g. "how do I write a Rust iterator?") does not mention any verification/completion trigger phrase and does not match the "claim work is complete" broad trigger — does not load.

Regenerated `.skills/really-done/SKILL.md` via `cargo install --path swissarmyhammer-cli` (embedded skills are `include_dir!`'d at compile time, so a rebuild is required before `sah init` picks up the change) followed by `sah init`. `sah validate` passes clean. #skills-guide