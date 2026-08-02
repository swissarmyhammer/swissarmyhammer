---
assignees:
- claude-code
position_column: todo
position_ordinal: e180
title: Merge seven single-rule builtin validators into one hygiene validator set
---
## What

The review fleet makes one agent task for each validator that matches the changed files. Seven builtin validators hold one concern each, and each one costs a full agent turn (~100K-token prime upload plus generation) per review run. In the five review runs on 2026-08-01, these seven produced zero findings: `no-secrets`, `injection`, `command-safety`, `no-commented-code`, `function-length`, `dead-code`, `test-integrity`. Merge them into one `hygiene` validator set so the fleet sends one task instead of seven.

The source of truth is `builtin/validators/` (deployed to `~/.validators` by `sah init`; see `builtin/validators/README.md` for the precedence rules). A set is a directory with a `VALIDATOR.md` manifest plus a `rules/` directory. Multi-rule sets are the established pattern: `builtin/validators/swift` has 11 rules, `python` has 8, `duplication` has 3.

Make these changes:

- Create `builtin/validators/hygiene/VALIDATOR.md`. The manifest needs:
  - `match.files`: `@file_groups/source_code` PLUS the test-file globs from `builtin/validators/test-integrity/VALIDATOR.md` (the test-cheating rules must still see test files).
  - `probes: [callers]` — the dead-code rule needs the call-graph evidence probe (see `builtin/validators/dead-code/VALIDATOR.md`).
- Move these eight rule files unchanged into `builtin/validators/hygiene/rules/`: `no-secrets.md`, `injection.md`, `command-safety.md`, `no-commented-code.md`, `function-length.md`, `dead-code.md`, and from test-integrity: `no-hard-code.md`, `no-test-cheating.md`.
- Delete the seven retired set directories from `builtin/validators/`.
- Make the builtin validator refresh remove a retired builtin set from the deployed store (`~/.validators`) — but only when the deployed files are identical to the shipped builtin content. Never remove a set the user changed or added. The deploy code is in `crates/mirdan/src/install.rs`; the embedded set list comes from `crates/mirdan/src/builtin_validators.rs` (`builtin_validators_by_set`).

Out of scope: `naming` and `magic-numbers` are user-level sets in `~/.validators` only (they do not exist in `builtin/validators/`). Leave them alone.

## Subtasks

- [ ] Create `builtin/validators/hygiene/VALIDATOR.md` with the union match globs and `probes: [callers]`
- [ ] Move the eight rule files into `builtin/validators/hygiene/rules/`
- [ ] Delete the seven retired set directories
- [ ] Remove retired, unmodified builtin sets from the deployed store on refresh (`crates/mirdan/src/install.rs`)
- [ ] Update the embed and loader tests (see Tests)

## Acceptance Criteria

- [ ] The validator loader reports a `hygiene` set with 8 rules, `probes: ["callers"]`, and match globs that include both source and test patterns
- [ ] The loader no longer reports the seven retired set names from the builtin layer
- [ ] A refresh deploy removes an unmodified retired set from the target store, and keeps a user-modified set of the same name
- [ ] Review behavior keeps all eight rules: the merged set ships the same rule texts the seven sets shipped

## Tests

- [ ] Update `test_builtin_validators_embed_expected_sets` in `crates/mirdan/src/builtin_validators.rs`: assert `hygiene` is present and the seven retired names are absent
- [ ] Update the loader tests in `crates/swissarmyhammer-validators/src/builtin/mod.rs` (they read `../../builtin/validators` directly): assert `hygiene` loads with `rule_count == 8` and the `callers` probe
- [ ] New test for the refresh prune in `crates/mirdan/src/install.rs`: deploy the old set, refresh, assert it is gone; deploy a modified copy, refresh, assert it stays
- [ ] Run `cargo test -p mirdan -p swissarmyhammer-validators` — all tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #review