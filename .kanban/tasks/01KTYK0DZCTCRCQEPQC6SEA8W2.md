---
assignees:
- claude-code
position_column: todo
position_ordinal: f380
title: 'Pre-existing failure: meta_tree_id_param_is_required_where_expected — unregister.id required flag is false'
---
## What

`cargo nextest run -p swissarmyhammer-command-service` has one pre-existing failure, unrelated to the perspectives work:

```
FAIL swissarmyhammer-command-service::meta_tree meta_tree_id_param_is_required_where_expected
panicked at crates/swissarmyhammer-command-service/tests/meta_tree.rs:68:9:
assertion `left == right` failed: unregister.id required flag
  left: Bool(false)
 right: true
```

Verified pre-existing: fails identically at HEAD (206ffbabf) with the perspectives-fix working tree stashed (2026-06-12, during task 01KTY6T1GPY94VYWANE9X41SKJ).

## Fix

The `command` tool's `_meta` discovery tree no longer marks the `unregister` op's `id` param as required. Either the op struct lost a required annotation or the meta-tree generator changed. Restore the required flag (or update the test if the relaxation was intentional and documented).

## Verify

`cargo nextest run -p swissarmyhammer-command-service` fully green.