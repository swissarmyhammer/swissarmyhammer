---
assignees:
- claude-code
position_column: todo
position_ordinal: ffde80
title: The review engine re-reviews moved content, because it sees a move as a delete plus an add
---
A change that relocates code into new modules reads to the review engine as thousands of deleted lines and thousands of added lines. Git detects renames; the review scope does not use that, so every moved line enters the work list as NEW content and is reviewed again.

## Measured on 2026-08-13, on ^0fn6dbf

That card split six over-cap files into module trees. `git diff --stat 56087bc1a~1..bb0b22897` reports **70 files changed, 20548 insertions, 20097 deletions**.

Almost none of that is new code. The six source files are EDITS — each sheds thousands of lines and gains one `mod` line:

| added | removed | file |
| --- | --- | --- |
| 1 | 3947 | `crates/swissarmyhammer-kanban/src/scope_commands.rs` |
| 1 | 3391 | `crates/swissarmyhammer-kanban/src/dispatch.rs` |
| 1 | 3283 | `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs` |
| 2 | 2835 | `crates/swissarmyhammer-config/src/model.rs` |
| 1 | 2364 | `crates/swissarmyhammer-entity/src/context.rs` |
| 33 | 4277 | `crates/swissarmyhammer-tools/tests/integration/file_tools_integrations.rs` |

The real semantic delta is about 39 lines of module wiring. The engine offered ~20000.

## What it cost

Two review attempts died on that range. The fan-out reached **175 concurrent `--model haiku` agents** under one `sah serve`, which stalled for 600s and had to be killed by hand; a third attempt only completed because its scope was cut by hand to 11 files.

A narrow review of the 11 files that actually changed finished, found the wiring correct, and reported 0 skipped files.

## Note the vocabulary too

This work was repeatedly described as a "pure move". That is wrong and the wrongness is the point: a relocation is a move of CONTENT plus an EDIT of the source file. The engine sees only the edit, and the review scope should say so plainly.

## What to do

- Use git's own rename and copy detection when resolving a scope, so relocated content is not re-reviewed as new.
- Where content moved unchanged, review the DELTA — the module wiring, the imports, the visibility — not the moved body.
- Report what was recognised as moved, so a silent scope reduction is impossible. An empty finding list on skipped content is the failure mode to avoid.
- Cap or stage the fan-out so a large scope degrades instead of spawning 175 agents.

## Done when

- A relocation commit reviews in proportion to its real delta, not its line count.
- The report states which files were recognised as relocated and what was reviewed in each.

#tool-validators