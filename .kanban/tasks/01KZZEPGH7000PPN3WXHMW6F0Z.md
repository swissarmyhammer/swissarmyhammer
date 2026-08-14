---
assignees:
- claude-code
position_column: todo
position_ordinal: ffdf80
title: 28 pre-existing findings in the six files ^0fn6dbf made reviewable
---
Splitting the six over-cap files on ^0fn6dbf made them readable by the review engine for the first time. The first narrow review of them returned **28 confirmed findings**, every one in PRE-EXISTING production code rather than in that card's own change. The new module scaffolding produced no surviving finding.

These are recorded here rather than on ^0fn6dbf because they are not that card's deliverable. ^0fn6dbf set out to bring the files under the 262144-byte cap, and it did — `skipped_files` was empty on all 11 review runs. Finding these defects is the POINT of that work, not a defect in it.

## The findings, by file

`crates/swissarmyhammer-entity/src/context.rs` — 50, 180, 222, 371, 438, 454 (two), 471, 742
`crates/swissarmyhammer-config/src/model.rs` — 349, 351, 482, 648, 915, 953, 1008, 1030, 1461, 1493, 1506
`crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs` — 1171, 1225, 1332, 1518, 1563, 2119, 2129
`crates/swissarmyhammer-kanban/src/dispatch.rs` — 750

The full text of each stands in the `## Review Findings` section of ^0fn6dbf, dated 2026-08-14.

## Verified, not assumed

The reviewer read every asserted line before recording. **0 findings were dropped for a false premise** — including the two "undocumented enum variant" claims at `model.rs:349,351`, where both variants genuinely carry no doc comment, and the "should implement Debug" claim at `context.rs:50`, where no `#[derive(Debug)]` is present. A further **18 findings were dropped** under the review skill's existing-tests exception, each confirmed present verbatim before the split with `git show <sha>^`.

## One caveat on the measurement

On all 11 runs the tool rules `duplication/duplication-parsed` and `code-hygiene/no-commented-code-parsed` reported `tool missing: bash: : command not found`, so prompt rules substituted and the deterministic duplication detector never ran. Those two rules were deleted from `builtin/` on ^wwb6hk7 but survive in the deployed `~/.validators/` store; ^w6ypb8b prunes them, and takes effect after a `sah` rebuild plus `sah init user`. So a re-run after that cleanup may report a different duplication picture.

## Done when

- Each finding is judged on its own evidence, fixed or recorded as a conflict.
- The four files re-review with no confirmed finding.

#tool-validators