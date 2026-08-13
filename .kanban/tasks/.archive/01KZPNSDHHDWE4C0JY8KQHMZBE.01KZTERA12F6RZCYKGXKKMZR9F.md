---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzteqyvr32krxk791hs24w1d
  text: |-
    Archived. The `duplication-parsed` rule is removed — see ^wwb6hk7.

    The stale measurement goes away with the rule file that states it.
  timestamp: 2026-08-12T07:42:13.880219+00:00
position_column: todo
position_ordinal: ffca80
title: 'duplication-parsed states a stale measurement: 416 findings over 1183 files, against 403 over 1191'
---
`builtin/validators/duplication/rules/duplication-parsed.md` states its gate measurement over "the 1183 tracked `.rs` files of this workspace" and reports **416** findings at 40 tokens and 90 percent.

Measured again on 2026-08-10 at HEAD 87a8c3da7 with a release `sah`: the tree holds **1191** tracked `.rs` files and the rule reports **403** findings. The rule body is stale by 8 files and 13 findings.

The stale number is not one cell. The whole rule body reads off it:

- The 7 x 5 calibration table of minimum tokens against similarity percent.
- "258 of the 416 findings are that case", for the equal-stream group.
- "A gate of 100 reports 258 of the 416".
- "The 416 are 395 functions, 13 structs and 8 enums or traits."
- The window-against-definitions table, which names 416 and 1183.
- "Over all 1183 tracked `.rs` files of this workspace: **416** findings. With the Rust test markers taken out of the table and nothing else changed: **3490**."

## What to do

Measure the whole table again with a release `sah` over the tracked `.rs` files of the tree at the time, and write each number the body states. Name the file count and the commit the measurement came from.

## Depends on the decision on ^qpjp9v9

^qpjp9v9 makes `write_shape` skip an attribute. That change moves the count to 386 on the same tree. Wait for the decision on that card, then measure once against the code that stands.

## Done when

- Every count in `duplication-parsed.md` comes from one measurement of one tree
- The body names the file count and the commit of that measurement #tool-validators #objectivity