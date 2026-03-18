---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffcc80
title: Remove AvpHooks component from sah init
---
## What

Remove the `AvpHooks` component from `sah init` so that AVP hook installation is only done via `avp init`.

### Files to modify
- `swissarmyhammer-cli/src/commands/install/components/mod.rs`:
  - Remove `registry.register(AvpHooks);` from `register_all_components()` (line 27)
  - Remove the entire `AvpHooks` struct and `impl Initializable for AvpHooks` block (lines 1224-1296)
  - Remove the 4 AvpHooks tests: `test_avp_hooks_applicable_project`, `test_avp_hooks_applicable_local`, `test_avp_hooks_not_applicable_user`, `test_avp_hooks_metadata` (lines 1374-1394)

### What NOT to change
- `avp-common/src/install.rs` — keep all shared install logic intact, `avp init` still uses it
- `avp-common` dependency in `swissarmyhammer-cli/Cargo.toml` — keep it, doctor checks use `avp_common::install::is_avp_hook()`
- `avp-cli/src/install.rs` — untouched, `avp init` continues to work as before

## Acceptance Criteria
- [ ] `sah init` no longer installs AVP hooks
- [ ] `sah deinit` no longer removes AVP hooks
- [ ] `avp init` still works independently
- [ ] No references to `AvpHooks` remain in `components/mod.rs`

## Tests
- [ ] Run `cargo test -p swissarmyhammer-cli` — all tests pass (removed tests no longer run, no other test breaks)
- [ ] Run `cargo test -p avp-common` — all tests still pass
- [ ] Run `cargo test -p avp-cli` — all tests still pass