---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffc080
title: Add AVP checks to sah doctor
---
## What

Add two new doctor checks to `swissarmyhammer-cli/src/commands/doctor/checks.rs`:

1. **`check_avp_in_path()`** — searches PATH for the `avp` binary. Warning if missing, with install instructions (`cargo install --path avp-cli`).
2. **`check_avp_hooks()`** — checks `.claude/settings.json` and `.claude/settings.local.json` in the current directory for AVP hooks using `avp_common::install::is_avp_hook()`. Warning if no hooks found, with fix message to run `avp init`.

Wire both into `Doctor::run_system_checks()` in `swissarmyhammer-cli/src/commands/doctor/mod.rs`.

### Files to modify
- `swissarmyhammer-cli/src/commands/doctor/checks.rs` — add `check_avp_in_path()` and `check_avp_hooks()`, add check name constants
- `swissarmyhammer-cli/src/commands/doctor/mod.rs` — call both new checks from `run_system_checks()`

### Reference implementation
- `avp-cli/src/doctor.rs` lines 71-152 has nearly identical logic for both checks — adapt to the `checks::check_*(&mut Vec<Check>)` pattern used by sah doctor
- `avp_common::install::is_avp_hook()` is the canonical hook detection function — reuse it

## Acceptance Criteria
- [ ] `sah doctor` shows "AVP in PATH" check: OK with path if found, Warning with install instructions if missing
- [ ] `sah doctor` shows "AVP Hooks Installed" check: OK listing scopes if found, Warning with `avp init` instructions if missing
- [ ] Both checks use Warning severity (not Error) since AVP is optional

## Tests
- [ ] Add `test_check_avp_in_path()` in `checks.rs` — verify check is produced with correct name
- [ ] Add `test_check_avp_hooks_empty()` in `checks.rs` — verify Warning when no settings files exist (use tempdir)
- [ ] Run `cargo test -p swissarmyhammer-cli` — all existing + new tests pass