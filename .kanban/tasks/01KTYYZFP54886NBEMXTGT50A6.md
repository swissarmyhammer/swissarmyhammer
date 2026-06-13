---
assignees:
- claude-code
position_column: todo
position_ordinal: f980
title: Consolidate hand-rolled atomic-write (temp sibling + rename) helpers into swissarmyhammer-common::fs_utils
---
## What

Review of the ui-state clobber-protection fix (01KTYVRR39X1TFFTSH87X62QH1) found the workspace now has three independent copies of the atomic-write pattern (write temp sibling → rename → cleanup-on-rename-failure):

- `crates/swissarmyhammer-common/src/fs_utils.rs` — `StdFileSystem::write_with_permissions` (trait-shaped, `SwissArmyHammerError`, and a `#[cfg(test)]` branch that writes NON-atomically in common's own test builds)
- `crates/swissarmyhammer-ui-state/src/state.rs` — `UIState::save` + `temp_sibling` (io::Result, pid+counter temp names)
- a near-verbatim private `temp_sibling` in `codegen.rs` (per review output)

Each copy gets hardened independently (e.g. the fs_utils version sets permissions before rename; the others do not).

## Proposed outcome

- Hoist a free function `write_atomic(path, contents) -> io::Result<()>` (temp-sibling mint + write + rename + cleanup) into `swissarmyhammer-common::fs_utils`, without the cfg(test) non-atomic escape hatch.
- Migrate `UIState::save` (add the `swissarmyhammer-common` dependency to `swissarmyhammer-ui-state`) and `codegen.rs::temp_sibling` to it.
- Keep ui-state's atomicity regression tests (`save_replaces_file_atomically_via_rename`, `concurrent_load_during_save_never_sees_torn_state`) green — they pin the behavior across the migration.

## Constraints

- Crate-scoped builds/tests only (common, ui-state, codegen's crate).