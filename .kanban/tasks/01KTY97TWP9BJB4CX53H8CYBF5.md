---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw04tw4e08rjpks2cnqxt4th
  text: |-
    Investigated — the failure NO LONGER REPRODUCES on current HEAD (ef627f6c0).

    Evidence:
    - `cargo nextest run -p swissarmyhammer-command-service meta_tree_id_param_is_required_where_expected` => 1 passed.
    - Full package `cargo nextest run -p swissarmyhammer-command-service` => 173 tests run, 173 passed, 0 skipped. (Card expected 142/142; suite has since grown to 173.)
    - `cargo clippy -p swissarmyhammer-command-service -p swissarmyhammer-operations` => clean, exit 0.

    Root cause analysis: The card was filed against an earlier HEAD (commit fbddb9566 per the session git snapshot). The `required` flag in the meta tree is derived directly from `param.required` of the ParamMeta for the `unregister` op's `id` param — see `params_to_meta` in crates/swissarmyhammer-operations/src/schema.rs, which sets `entry["required"] = json!(param.required)`. The current operations source (crates/swissarmyhammer-command-service/src/operations.rs) correctly declares `id` as required. The origin/main merge `e6595fbce` (the most recent change touching operations.rs/schema.rs) carries the corrected schema, so the regression the card describes is already resolved upstream.

    No production code change needed; no source files touched. The test correctly asserts `id["required"] == true` and that now holds.
  timestamp: 2026-06-25T19:41:42.926212+00:00
- actor: claude-code
  id: 01kw04vndkdhezvsge7gfh74cc
  text: 'Verified resolved on current HEAD — no code change needed. `cargo nextest run meta_tree_id_param_is_required_where_expected` passes; full `swissarmyhammer-command-service` package 173/173 green; clippy clean. Root cause: the meta tree''s `required` flag derives from `ParamMeta.required` for unregister''s `id` param, which is now declared required in the operations source (corrected by origin/main merge e6595fbce after this card was filed at fbddb9566). The test expectation (`id["required"] == true`) is correct and holds. Stale pre-existing-failure card; nothing to review (no code delta). Moving to done. NOTE: identical duplicate 6sea8w2 (01KTYK0DZCTCRCQEPQC6SEA8W2) is resolved by the same fact.'
  timestamp: 2026-06-25T19:42:08.819842+00:00
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffff080
title: 'Pre-existing failure: meta_tree_id_param_is_required_where_expected — unregister.id required flag is false'
---
## What

`cargo nextest run -p swissarmyhammer-command-service` has ONE pre-existing failure, discovered while verifying card 01KTY6XTJQFCG9ENKTAMC6N3JV (which touched only caption.rs / lib.rs export / inspect tests — `git diff HEAD` for the failing test and its subjects is empty, so this fails on committed HEAD):

```
thread 'meta_tree_id_param_is_required_where_expected' panicked at crates/swissarmyhammer-command-service/tests/meta_tree.rs:68:9:
assertion `left == right` failed: unregister.id required flag
  left: Bool(false)
 right: true
```

## Where

- Test: `crates/swissarmyhammer-command-service/tests/meta_tree.rs` — asserts `meta["command"][verb]["parameters"]["id"]["required"] == true` for verbs unregister/schema/execute/available.
- Subject: `generate_operations_meta` (`crates/swissarmyhammer-operations/src/schema.rs`, last changed in merge 74ed5e2f0 "Merge origin/main into plugin" / 4f0997e02 "Tools (#60)") over `operations()` from `crates/swissarmyhammer-command-service/src/operations.rs`.

## Suspect

The origin/main merge changed how `generate_operations_meta` computes the `required` flag (or the operations schema derive) so `unregister`'s `id` no longer reports required. Determine whether the meta generator regressed or the schema declaration needs a required marker, fix at the source, and re-run `cargo nextest run -p swissarmyhammer-command-service` (must be 142/142).