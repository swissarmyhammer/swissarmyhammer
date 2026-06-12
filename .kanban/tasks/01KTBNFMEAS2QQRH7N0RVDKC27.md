---
assignees:
- claude-code
depends_on: []
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffeb80
project: local-review
title: 'Teardown: remove the AVP validator doctor rule'
---
## What
Remove the doctor check that verifies the AVP validator directory, since AVP-as-hooks is being retired and the validator directories are changing (later task moves them to `~/.validators` / `./.validators`).

- Remove `check_avp_directory()` and its registration from `crates/mirdan/src/doctor.rs` (the check that looks for `.avp/` project dir and `$XDG_DATA_HOME/avp/` global dir, ~lines 174–205), plus its test `test_check_avp_directory` (~line 331).
- Remove any `AvpConfig`/`ManagedDirectory::<AvpConfig>` usage that becomes dead once the check is gone, if it is not used elsewhere.
- `apps/avp-cli/src/doctor.rs` is deleted with the avp-cli app (separate teardown task) — do not duplicate that work here.
- Do NOT add a replacement `.validators` doctor check in this task; if one is wanted it belongs with the directory-relocation/install task.

## Acceptance Criteria
- [ ] `check_avp_directory` and its test are gone from `crates/mirdan/src/doctor.rs`; the doctor still compiles and runs.
- [ ] No dangling references to the removed check; `AvpConfig` either still has a legitimate consumer or is also removed.
- [ ] `mirdan doctor` runs and reports the remaining checks without the AVP-directory line.

## Tests
- [ ] `cargo test -p mirdan doctor` green after removal.
- [ ] Existing mirdan doctor test suite passes with the AVP check removed (update any snapshot/count assertions that included it).

## Workflow
- Use `/tdd` only if a count/snapshot test needs updating first; otherwise a surgical removal verified by `cargo test -p mirdan`. Reuse the existing `DoctorRunner` pattern; don't restructure the doctor.