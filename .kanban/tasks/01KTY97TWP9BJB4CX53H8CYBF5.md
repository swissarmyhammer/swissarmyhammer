---
assignees:
- claude-code
position_column: todo
position_ordinal: f180
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